# AIDoc

AI-native structured document format — implementation of the
[AIDoc v0.1 Format Specification](./docs/AIDoc%20v0.1%20Format%20Specification.md).

Every edit is a typed `Operation`; every operation produces a `Revision`;
every revision is immutable. The Rust core persists to SQLite inside a
single `.aidoc` ZIP package, and an MCP server exposes the same operation
engine to AI agents. A Tauri 2 + React desktop wraps the core, and a Python
AI agent connects through the MCP server.

## Quickstart

```sh
# Build everything (Rust workspace + prebuilt MCP binary for the AI agent)
cargo build --workspace

# CLI: run the demo (create → update → revert, dump HTML)
cargo run -p aidoc-cli -- demo examples/order-system.aidoc \
    --export-html examples/order-system.html

# CLI: scaffold one of the bundled templates
cargo run -p aidoc-cli -- example order-system examples/order-system.aidoc
cargo run -p aidoc-cli -- example api-system    examples/api-system.aidoc
cargo run -p aidoc-cli -- example knowledge-graph examples/knowledge-graph.aidoc

# MCP server (line-delimited JSON-RPC over stdio)
cargo run -p aidoc-mcp

# Desktop UI (Tauri 2 + React + shadcn/ui + Tiptap + Mermaid)
cargo tauri dev

# AI agent (Python) — see apps/ai/README.md
pip install httpx openai
python apps/ai/agent.py --mcp-bin target/debug/aidoc-mcp.exe \
    --doc examples/order-system.aidoc \
    --prompt "List every node and summarize this document."
```

## CLI

| Command | Purpose |
| --- | --- |
| `init <path>` | Create an empty `.aidoc` package. |
| `info <path>` | Document title, root, head revision. |
| `node-list <path>` | List every node in the current document. |
| `node-show <path> <id>` | Print one node's content + metadata. |
| `apply <path> [--op-file <json>]` | Apply an Operation JSON (file or stdin). |
| `history <path> [--json]` | Print revision history. |
| `diff <path> [from] [to]` | Per-revision snapshot diff (add / remove / change). |
| `revert <path> <target>` | Roll back to a past revision (creates a new revision). |
| `validate <path>` | Run all 5 validators (identity / structure / relation / revision / code-ref). |
| `export <path> <out> [--format html\|md]` | Export to HTML or Markdown. |
| `demo <path> [--export-html <out>]` | Run a full create → update → revert loop. |
| `example <template> <path>` | Scaffold `order-system` / `api-system` / `knowledge-graph`. |
| `branch <path> <name>` | Tag the current head with a named branch. |
| `merge <path> <branch>` | Merge a named branch back into main. |

## Desktop

`apps/desktop` is a Tauri 2 + React 18 + Vite app:

- **Tree** of every node — search, sort, drag to reorder, right-click for actions.
- **13 node kinds** — Section / Heading / Paragraph / List / Code / Quote / Diagram
  / Code-ref / Requirement / Decision / Problem / Solution / Reference.
- **Tiptap rich-text editor** with bold / italic / headings / lists / quote / code,
  wrapped in a kind-aware shell that colors Requirement / Decision / Problem / Solution.
- **Mermaid diagrams** with auto light/dark theme.
- **Command palette** — `⌘K` (or `Ctrl+K`) opens a cmdk palette with three
  groups: Document (save / export / validate), Nodes (search + jump), Revisions
  (view diff / revert).
- **Per-revision diff dialog** — see every `Change` (created / updated / moved /
  renamed / reverted / linked / unlinked) for a single revision.
- **Global shortcuts** — `⌘K` palette, `⌘S` save, `⌘E` export HTML, `⌘N` new section.
- **Settings** — autosave, editor font size (S/M/L), Mermaid theme (auto /
  Light / Dark), and AI (OpenAI API key / base URL / model).
- **Ask AIDoc AI** — `✨` in the header opens a chat panel that spawns the Python
  agent against the running MCP server. Multi-turn, history persisted to
  localStorage, node-id mentions become clickable links to the tree, and
  "Export" downloads the conversation as Markdown.

## Architecture

```text
React + shadcn/ui + Tiptap + Mermaid   (apps/desktop/ui)
         │
         ▼
Tauri 2 commands (init_doc / open_doc / list_nodes / update_node /
                  create_node / delete_node / move_node / set_node_kind /
                  list_changes / validate_aidoc / ai_chat / …)
         │
         ▼
Rust Core  (crates/aidoc-model / -storage / -operation / -history
            / -validator / -package / -renderer / -exporter)
         │
         ▼
SQLite inside a single ZIP package  (.aidoc)

Python AI Agent  ──stdios──▶  aidoc-mcp  ──▶  Rust Core
(openai-compatible / Ollama / GLM / DeepSeek / …)
```

## Layout

```
aidoc/
├── apps/
│   ├── ai/                   Python MCP client + OpenAI agent (CLI + smoke test)
│   ├── desktop/              Tauri 2 + React + Tiptap editor
│   │   ├── src-tauri/            Rust-side Tauri commands
│   │   └── ui/                  React frontend (shadcn/ui + Tailwind + Radix + lucide)
│   └── cli/                  aidoc CLI binary
├── crates/
│   ├── aidoc-model/      Document / Node / Relation / Revision / Operation / Provenance
│   ├── aidoc-storage/    SQLite schema + transaction wrapper
│   ├── aidoc-operation/  Operation engine + optimistic concurrency + conflict detection
│   ├── aidoc-history/    Revert (always produces a new revision)
│   ├── aidoc-validator/  Identity / Structure / Relation / Revision / CodeRef
│   ├── aidoc-package/    ZIP .aidoc reader/writer + manifest + temp workspace
│   ├── aidoc-renderer/   AIDoc → HTML (TOC, dark theme, requirement/decision/problem/solution)
│   ├── aidoc-exporter/   HTML / Markdown export
│   ├── aidoc-mcp/        stdio MCP server (16 tools, see below)
│   └── aidoc/            Facade: re-exports every sub-crate
├── docs/                  Format spec + dev notes
└── examples/              Bundled example documents
```

## 4 MUST Rules

1. Every node has a stable ID.
2. Mutations only go through `Operation`.
3. Every operation produces a `Revision`.
4. Revert creates a new revision (history is immutable).

## MCP server

`aidoc-mcp` speaks line-delimited JSON-RPC over stdio (rmcp 0.3.2).

Tools (16):

```
init_aidoc   open_aidoc    save_aidoc    list_nodes    show_node
search_nodes create_node   update_node   delete_node   apply_operation
history      revert        export_html   validate      branch
merge
```

Wire handshake: send `initialize` → receive server info → send
`notifications/initialized` → `tools/list` / `tools/call …`.

Python client: `apps/ai/agent.py` (or `apps/ai/smoke_test.py` for a no-API-key
handshake check). See `apps/ai/README.md`.

## CI

`.github/workflows/ci.yml` runs three jobs on every PR:

1. `rust` matrix (Ubuntu + Windows): `cargo fmt --check`, `cargo clippy -D warnings`,
   `cargo test --workspace`, plus release builds of `aidoc-cli` and `aidoc-mcp`.
2. `ui`: `pnpm install` + `pnpm build` for the React + Tiptap frontend.
3. `integration-smoke`: `cargo test -p aidoc-mcp --test stdio_smoke` end-to-end
   handshake over stdio.