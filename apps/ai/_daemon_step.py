"""Step-by-step daemon debug: time each phase."""
import sys, time
from pathlib import Path
sys.path.insert(0, str(Path("apps/ai").resolve()))
from agent import MCPClient, initialize, call_tool, list_tools

t0 = time.monotonic()
print(f"[{time.monotonic()-t0:.2f}s] spawning aidoc-mcp...", flush=True)
mcp = MCPClient(["target/debug/aidoc-mcp.exe"])
print(f"[{time.monotonic()-t0:.2f}s] spawned", flush=True)

print(f"[{time.monotonic()-t0:.2f}s] initialize...", flush=True)
initialize(mcp)
print(f"[{time.monotonic()-t0:.2f}s] initialized", flush=True)

print(f"[{time.monotonic()-t0:.2f}s] list_tools...", flush=True)
tools = list_tools(mcp)
print(f"[{time.monotonic()-t0:.2f}s] {len(tools)} tools", flush=True)

print(f"[{time.monotonic()-t0:.2f}s] open_aidoc...", flush=True)
out = call_tool(mcp, "open_aidoc", {"path": "examples/order-system.aidoc"})
print(f"[{time.monotonic()-t0:.2f}s] opened, {len(out)} bytes", flush=True)

print(f"[{time.monotonic()-t0:.2f}s] closing", flush=True)
mcp.close()
print(f"[{time.monotonic()-t0:.2f}s] done", flush=True)