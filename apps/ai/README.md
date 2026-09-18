# AIDoc AI agent

Standalone Python client that connects to the Rust `aidoc-mcp` server over
stdio, lists AIDoc tools, and runs a single-turn tool-calling loop against an
OpenAI-compatible HTTP API (works with OpenAI, Ollama, GLM, DeepSeek, etc.).

This is the **AI / RAG link** the spec §11 / §13 calls for. The Rust core
stays in charge of mutations; the agent can only act through the same MCP
tools the CLI uses, so every AI edit is still an `Operation` that produces a
new `Revision`.

## Install

```sh
# Python 3.11+; httpx and openai are the only deps
pip install httpx openai
```

## Run

```sh
export OPENAI_API_KEY=sk-...
python apps/ai/agent.py \
    --mcp-bin "cargo run -p aidoc-mcp --" \
    --doc examples/order-system.aidoc \
    --prompt "List every node in this document and tell me which ones are placeholders."
```

Custom endpoint (Ollama, vLLM, GLM, …):

```sh
export OPENAI_BASE_URL=http://localhost:11434/v1
export OPENAI_MODEL=llama3.1
python apps/ai/agent.py --prompt "Summarize this doc." --doc examples/order-system.aidoc
```

## How it works

1. Spawn `aidoc-mcp` as a subprocess, attach to its stdio.
2. Speak the **Model Context Protocol** (Content-Length framed JSON-RPC):
   `initialize` → `notifications/initialized` → `tools/list` → `tools/call`.
3. Translate the tool list into OpenAI function-calling format.
4. Loop:
   - Send messages + tools to the LLM.
   - If the response carries `tool_calls`, dispatch each one via MCP,
     append the tool result, and ask the LLM again.
   - Stop when the LLM returns plain text (or after `max_steps`).

## Environment

| Var | Default | Purpose |
|---|---|---|
| `OPENAI_API_KEY` | (required) | Bearer token |
| `OPENAI_BASE_URL` | `https://api.openai.com/v1` | Any OpenAI-compatible host |
| `OPENAI_MODEL` | `gpt-4o-mini` | Model id |

## Limits of this cut

- Single-turn agent; no persistent conversation history.
- No RAG indexing yet — the LLM gets whatever `list_nodes` /
  `show_node` / `list_changes` / `history` returns. FTS5 + a vector
  store come in a later pass.
- No Tauri integration in this pass; this is the **CLI brick**. The
  next pass wires it into the desktop UI via a Tauri command and an
  "Ask AI" panel.