#!/usr/bin/env python3
"""Minimal daemon repro."""
import sys

print("STDIN is:", type(sys.stdin).__name__, flush=True)
print("STDIN readable:", sys.stdin.readable(), flush=True)

i = 0
for line in sys.stdin:
    print(f"got line {i}: {line!r}", flush=True)
    i += 1
    if i > 5:
        break
print("done", flush=True)