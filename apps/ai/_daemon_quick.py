#!/usr/bin/env python3
"""Quick manual check: start daemon, send one request, print response + stderr."""
from __future__ import annotations

import json
import subprocess
import sys
import time

if __name__ == "__main__":
    p = subprocess.Popen(
        ["python", "apps/ai/agent.py", "--daemon", "--mcp-bin", "target/debug/aidoc-mcp.exe"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        bufsize=0,
    )
    assert p.stdin and p.stdout and p.stderr
    p.stdin.write((json.dumps({"id": 1, "prompt": "noop", "doc_path": "examples/order-system.aidoc", "api_key": "sk-test"}) + "\n").encode())
    p.stdin.flush()
    # Read stderr + stdout for a moment
    start = time.monotonic()
    line = b""
    while time.monotonic() - start < 5:
        try:
            chunk = p.stdout.readline()
            if chunk:
                line = chunk
                break
        except Exception:
            break
        time.sleep(0.05)
    p.terminate()
    err = p.stderr.read()
    print("STDOUT:", line.decode("utf-8", errors="replace"))
    print("STDERR:", err.decode("utf-8", errors="replace"))