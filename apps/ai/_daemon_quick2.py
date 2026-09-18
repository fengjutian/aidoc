"""Spawn daemon, capture stderr, sleep 3s, print stderr."""
import subprocess, sys, time

p = subprocess.Popen(
    ["python", "apps/ai/agent.py", "--daemon", "--mcp-bin", "target/debug/aidoc-mcp.exe"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    bufsize=0,
)
time.sleep(3)
if p.poll() is None:
    print("[host] daemon still alive after 3s", flush=True)
    p.terminate()
    p.wait(timeout=2)
else:
    print(f"[host] daemon exited with code {p.returncode}", flush=True)
err = p.stderr.read() if p.stderr else b""
print("--- STDERR ---")
print(err.decode("utf-8", errors="replace"))