#!/usr/bin/env python3
"""Smoke test: spawn aidoc-mcp, complete the MCP handshake, list tools,
and call `open_aidoc` on the bundled order-system example.

Does NOT require an API key. Verifies the Python MCP transport actually
talks to the Rust server.

Usage:
    python apps/ai/smoke_test.py
    python apps/ai/smoke_test.py --mcp-bin <path-or-command>
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

# Allow `python apps/ai/smoke_test.py` without installing the package.
sys.path.insert(0, str(Path(__file__).parent))
from agent import MCPClient, call_tool, initialize, list_tools  # noqa: E402


def main() -> int:
    p = argparse.ArgumentParser(description="MCP handshake smoke test")
    p.add_argument(
        "--mcp-bin",
        default="target/debug/aidoc-mcp.exe",
        help="Path to aidoc-mcp binary (default: target/debug/aidoc-mcp.exe)",
    )
    p.add_argument(
        "--doc",
        default="examples/order-system.aidoc",
        help="Sample .aidoc document to open",
    )
    args = p.parse_args()

    mcp_bin = args.mcp_bin
    doc = args.doc

    print(f"[smoke] spawning {mcp_bin}", file=sys.stderr)
    mcp = MCPClient([mcp_bin])
    try:
        initialize(mcp)
        print("[smoke] initialized ✓", file=sys.stderr)

        tools = list_tools(mcp)
        names = [t["function"]["name"] for t in tools]
        print(f"[smoke] {len(tools)} tools: {names}", file=sys.stderr)
        assert "list_nodes" in names, "list_nodes missing from tool registry"
        assert "open_aidoc" in names, "open_aidoc missing from tool registry"
        assert "show_node" in names, "show_node missing from tool registry"
        assert "search_nodes" in names, "search_nodes missing from tool registry"

        if Path(doc).exists():
            print(f"[smoke] opening {doc}", file=sys.stderr)
            out = call_tool(mcp, "open_aidoc", {"path": doc})
            assert "title" in out.lower() or "demo" in out.lower() or len(out) > 0, (
                f"open_aidoc returned empty: {out!r}"
            )
            print(f"[smoke] open_aidoc ✓ ({len(out)} bytes)", file=sys.stderr)

            nodes_text = call_tool(mcp, "list_nodes", {})
            print(f"[smoke] list_nodes ✓ ({len(nodes_text)} bytes)", file=sys.stderr)

            # Search for something that's very likely in a real doc.
            search = call_tool(mcp, "search_nodes", {"query": "system", "limit": 5})
            search_rows = json.loads(search)
            print(
                f"[smoke] search_nodes('system') ✓ {len(search_rows)} rows",
                file=sys.stderr,
            )

            # Empty query must return [] not every node.
            empty = call_tool(mcp, "search_nodes", {"query": "", "limit": 50})
            empty_rows = json.loads(empty)
            print(
                f"[smoke] search_nodes('') ✓ {len(empty_rows)} rows (expect 0)",
                file=sys.stderr,
            )
            assert len(empty_rows) == 0, (
                f"empty query should return [], got {empty[:200]}"
            )
        else:
            print(f"[smoke] doc {doc} not found, skipping tool calls", file=sys.stderr)

        print("[smoke] OK", file=sys.stderr)
        return 0
    finally:
        mcp.close()


if __name__ == "__main__":
    sys.exit(main())