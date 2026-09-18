#!/usr/bin/env python3
"""AIDoc AI agent — minimal MCP + OpenAI client.

Connects to an `aidoc-mcp` subprocess via stdio (Content-Length framed
JSON-RPC, per the Model Context Protocol spec), lists the AIDoc tools,
and runs a single-turn tool-calling loop against an OpenAI-compatible
HTTP API (works with OpenAI, Ollama, GLM, DeepSeek, etc.).

Usage:
    OPENAI_API_KEY=sk-... python agent.py \
        --mcp-bin "cargo run -p aidoc-mcp --" \
        --doc examples/order-system.aidoc \
        --prompt "List every node and summarize this document."

Environment:
    OPENAI_API_KEY    required (or set via --api-key)
    OPENAI_BASE_URL   default https://api.openai.com/v1
    OPENAI_MODEL      default gpt-4o-mini
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import threading
from queue import Empty, Queue
from typing import Any

import httpx


# ---------- MCP transport: stdio + Content-Length framing ----------


class MCPClient:
    """Minimal Model Context Protocol client over stdio subprocess.

    Wire format: line-delimited JSON (one JSON object per line, terminated
    by a newline). The Rust `aidoc-mcp` server speaks this format on its
    stdin/stdout.
    """

    def __init__(self, bin_cmd: list[str]) -> None:
        self.proc = subprocess.Popen(
            bin_cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            bufsize=0,
        )
        self._id = 0
        self._responses: dict[int, dict] = {}
        self._notifications: Queue[dict] = Queue()
        self._reader = threading.Thread(target=self._read_loop, daemon=True)
        self._reader.start()

    def _read_loop(self) -> None:
        """Read one JSON-RPC message per newline off the subprocess stdout."""
        stream = self.proc.stdout
        assert stream is not None
        for raw in stream:
            line = raw.decode("utf-8", errors="replace").strip()
            if not line:
                continue
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                continue
            if "id" in msg and ("result" in msg or "error" in msg):
                self._responses[msg["id"]] = msg
            else:
                self._notifications.put(msg)

    def _send(self, message: dict) -> None:
        assert self.proc.stdin is not None
        line = (json.dumps(message) + "\n").encode("utf-8")
        self.proc.stdin.write(line)
        self.proc.stdin.flush()

    def request(self, method: str, params: dict | None = None, timeout: float = 30.0) -> dict:
        self._id += 1
        msg_id = self._id
        payload: dict[str, Any] = {"jsonrpc": "2.0", "id": msg_id, "method": method}
        if params is not None:
            payload["params"] = params
        self._send(payload)
        import time

        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if msg_id in self._responses:
                return self._responses.pop(msg_id)
            time.sleep(0.05)
        raise TimeoutError(f"MCP request {method} timed out after {timeout}s")

    def notify(self, method: str, params: dict | None = None) -> None:
        payload: dict[str, Any] = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            payload["params"] = params
        self._send(payload)

    def close(self) -> None:
        try:
            if self.proc.stdin:
                self.proc.stdin.close()
        except OSError:
            pass
        self.proc.terminate()
        try:
            self.proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            self.proc.kill()


# ---------- MCP protocol helpers ----------


def initialize(mcp: MCPClient) -> None:
    """Run the MCP initialize → initialized handshake."""
    mcp.request(
        "initialize",
        {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "aidoc-ai", "version": "0.1.0"},
        },
    )
    mcp.notify("notifications/initialized")


def list_tools(mcp: MCPClient) -> list[dict]:
    """Return the server's tool list in OpenAI function-calling shape."""
    resp = mcp.request("tools/list")
    tools = resp.get("result", {}).get("tools", [])
    out: list[dict] = []
    for t in tools:
        out.append(
            {
                "type": "function",
                "function": {
                    "name": t["name"],
                    "description": t.get("description", ""),
                    "parameters": t.get("inputSchema", {"type": "object", "properties": {}}),
                },
            }
        )
    return out


def call_tool(mcp: MCPClient, name: str, arguments: dict) -> str:
    """Invoke a tool and return its text content as a single string."""
    resp = mcp.request(
        "tools/call", {"name": name, "arguments": arguments}, timeout=60.0
    )
    result = resp.get("result", {})
    parts: list[str] = []
    for item in result.get("content", []):
        if item.get("type") == "text":
            parts.append(item.get("text", ""))
        else:
            parts.append(json.dumps(item))
    return "\n".join(parts) if parts else "(no content)"


# ---------- LLM client (OpenAI-compatible HTTP) ----------


def chat_completion(
    client: httpx.Client,
    base_url: str,
    api_key: str,
    model: str,
    messages: list[dict],
    tools: list[dict],
) -> dict:
    """Call POST {base_url}/chat/completions with the agent's messages."""
    url = base_url.rstrip("/") + "/chat/completions"
    body: dict[str, Any] = {
        "model": model,
        "messages": messages,
        "temperature": 0.2,
    }
    if tools:
        body["tools"] = tools
        body["tool_choice"] = "auto"
    headers = {"Authorization": f"Bearer {api_key}"}
    resp = client.post(url, json=body, headers=headers, timeout=60.0)
    resp.raise_for_status()
    return resp.json()


# ---------- Agent loop ----------


def run_agent(
    mcp: MCPClient,
    base_url: str,
    api_key: str,
    model: str,
    prompt: str,
    system: str | None = None,
    max_steps: int = 6,
) -> str:
    """Drive the model in a single-turn tool-call loop."""
    tools = list_tools(mcp)
    messages: list[dict] = []
    if system:
        messages.append({"role": "system", "content": system})
    messages.append({"role": "user", "content": prompt})

    with httpx.Client() as client:
        for step in range(max_steps):
            print(f"[agent] step {step + 1}/{max_steps}", file=sys.stderr)
            resp = chat_completion(client, base_url, api_key, model, messages, tools)
            choice = resp["choices"][0]
            msg = choice["message"]
            messages.append(msg)

            tool_calls = msg.get("tool_calls") or []
            if not tool_calls:
                return msg.get("content", "")

            for tc in tool_calls:
                fn = tc.get("function", {})
                name = fn.get("name", "")
                raw_args = fn.get("arguments", "{}")
                try:
                    args = json.loads(raw_args) if isinstance(raw_args, str) else raw_args
                except json.JSONDecodeError:
                    args = {}
                print(f"[agent] tool call: {name}({args})", file=sys.stderr)
                try:
                    output = call_tool(mcp, name, args)
                except Exception as exc:  # noqa: BLE001
                    output = f"tool error: {exc}"
                messages.append(
                    {
                        "role": "tool",
                        "tool_call_id": tc.get("id", ""),
                        "content": output[:8000],
                    }
                )
    return messages[-1].get("content", "(no final answer)")


# ---------- CLI ----------


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="AIDoc AI agent (MCP + OpenAI)")
    p.add_argument(
        "--mcp-bin",
        default="cargo run -p aidoc-mcp --",
        help="Command to spawn aidoc-mcp (default: cargo run -p aidoc-mcp --)",
    )
    p.add_argument(
        "--doc",
        help="Path to a .aidoc document to open before chatting (passed to MCP server).",
    )
    p.add_argument("--prompt", required=True, help="User prompt")
    p.add_argument(
        "--api-key",
        default=os.environ.get("OPENAI_API_KEY"),
        help="OpenAI API key (default: $OPENAI_API_KEY)",
    )
    p.add_argument(
        "--base-url",
        default=os.environ.get("OPENAI_BASE_URL", "https://api.openai.com/v1"),
        help="OpenAI-compatible base URL",
    )
    p.add_argument(
        "--model",
        default=os.environ.get("OPENAI_MODEL", "gpt-4o-mini"),
        help="Model name",
    )
    p.add_argument(
        "--system",
        default=(
            "You are an AIDoc assistant. Use the provided tools to read and edit "
            "the user's .aidoc document. Answer concisely."
        ),
        help="System prompt",
    )
    return p.parse_args()


def main() -> int:
    args = parse_args()
    if not args.api_key:
        print(
            "error: OPENAI_API_KEY not set. Pass --api-key or export OPENAI_API_KEY.",
            file=sys.stderr,
        )
        return 2

    bin_cmd = args.mcp_bin.split()
    print(f"[agent] spawning: {bin_cmd}", file=sys.stderr)
    mcp = MCPClient(bin_cmd)
    try:
        initialize(mcp)
        # If --doc given, open it via the MCP tool so subsequent reads work.
        if args.doc:
            print(f"[agent] opening: {args.doc}", file=sys.stderr)
            call_tool(mcp, "open_aidoc", {"path": args.doc})
        answer = run_agent(
            mcp,
            base_url=args.base_url,
            api_key=args.api_key,
            model=args.model,
            prompt=args.prompt,
            system=args.system,
        )
        print(answer)
        return 0
    finally:
        mcp.close()


if __name__ == "__main__":
    sys.exit(main())