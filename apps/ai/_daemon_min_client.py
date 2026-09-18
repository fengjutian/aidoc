#!/usr/bin/env python3
import subprocess, sys, time
p = subprocess.Popen(["python", "apps/ai/_daemon_min.py"],
                     stdin=subprocess.PIPE,
                     stdout=subprocess.PIPE,
                     stderr=subprocess.STDOUT,
                     bufsize=0)
p.stdin.write(b"hello\nworld\n")
p.stdin.flush()
p.stdin.close()
out = p.stdout.read()
print("===OUTPUT===")
print(out.decode())
print("===END===", "rc=", p.wait())