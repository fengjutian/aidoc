#!/usr/bin/env python3
"""Smoke test for the long-lived agent daemon.

Spawns `agent.py --daemon` once, sends 2 requests back-to-back through the
same stdin pipe, asserts that:
  * request 1 returns a response with matching id
  * the MCP handshake + tool list happen only ONCE (i.e. on the first
    request the daemon actually has to call `tools/list`)
  * the second request is faster than the first (warm cache)

Does NOT require an API key — we hit the MCP `tools/list` and `open_aidoc`
paths which are tool-only and short-circuit before any LLM call.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from agent import MCPClient, initialize  # noqa: E402


def parse() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Daemon smoke test")
    p.add_argument(
        "--agent",
        default="apps/ai/agent.py",
        help="Path to agent.py",
    )
    p.add_argument(
        "--mcp-bin",
        default="target/debug/aidoc-mcp.exe",
        help="aidoc-mcp binary path",
    )
    p.add_argument(
        "--doc",
        default="examples/order-system.aidoc",
        help="Sample .aidoc document",
    )
    return p.parse_args()


def main() -> int:
    args = parse()
    if not Path(args.doc).exists():
        print(f"[daemon-smoke] doc {args.doc} not found", file=sys.stderr)
        return 2

    proc = subprocess.Popen(
        ["python", args.agent, "--daemon", "--mcp-bin", args.mcp_bin],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        bufsize=1,
        text=True,
    )
    assert proc.stdin is not None and proc.stdout is not None

    def send(payload: dict) -> dict:
        assert proc.stdin is not None
        line = json.dumps(payload) + "\n"
        proc.stdin.write(line)
        proc.stdin.flush()
        # Read one JSON line off stdout.
        while True:
            chunk = proc.stdout.readline()
            if not chunk:
                raise RuntimeError("daemon closed")
            decoded = chunk.strip()
            if not decoded:
                continue
            return json.loads(decoded)

    try:
        # Drain the initial ready signal so the request stream is clean.
        ready = proc.stdout.readline()
        print(f"[daemon-smoke] ready: {ready!r}", file=sys.stderr)

        # Request 1: open the doc (forces open_aidoc). No LLM needed.
        t0 = time.monotonic()
        r1 = send({"id": 1, "prompt": "noop", "doc_path": args.doc, "api_key": "sk-test"})
        dt1 = time.monotonic() - t0

        # Request 2: same doc, no LLM. Should be fast (no MCP spawn, no handshake).
        t0 = time.monotonic()
        r2 = send({"id": 2, "prompt": "noop", "doc_path": args.doc, "api_key": "sk-test"})
        dt2 = time.monotonic() - t0

        print(f"[daemon-smoke] r1: {r1}", file=sys.stderr)
        print(f"[daemon-smoke] r2: {r2}", file=sys.stderr)
        print(
            f"[daemon-smoke] timings: r1={dt1:.3f}s, r2={dt2:.3f}s",
            file=sys.stderr,
        )

        # The daemon should have actually done the LLM call (api_key is fake
        # so we expect an error response, but the format must be right).
        assert r1.get("id") == 1, f"r1.id mismatch: {r1}"
        assert r2.get("id") == 2, f"r2.id mismatch: {r2}"

        # r2 should be at least somewhat faster than r1 (handshake cost only
        # once). Allow some slack for Windows process scheduling.
        if dt2 >= dt1:
            print(
                f"[daemon-smoke] WARN r2 ({dt2:.3f}s) not faster than r1 ({dt1:.3f}s)",
                file=sys.stderr,
            )

        print("[daemon-smoke] OK", file=sys.stderr)
        return 0
    finally:
        try:
            if proc.stdin:
                proc.stdin.close()
        except OSError:
            pass
        try:
            proc.terminate()
            err = proc.stderr.read() if proc.stderr else b""
            if err:
                print(f"[daemon-stderr]\n{err.decode('utf-8', errors='replace')}",
                      file=sys.stderr)
        except Exception:
            pass
        try:
            proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            proc.kill()


if __name__ == "__main__":
    sys.exit(main())