#!/usr/bin/env python3
"""Pure echo daemon: no MCP, no LLM. Just reads lines from stdin, echoes to stdout."""
import sys
print("[pure-daemon] started", flush=True)
i = 0
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    print(f"[pure-daemon] got: {line}", flush=True)
    sys.stdout.write(f'{{"id":{i},"echo":{line}}}\n')
    sys.stdout.flush()
    i += 1
print("[pure-daemon] done", flush=True)