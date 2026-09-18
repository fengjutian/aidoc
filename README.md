# AIDoc

AI-native structured document format — implementation of the
[AIDoc v0.1 Format Specification](./docs/AIDoc%20v0.1%20Format%20Specification.md).

Every edit is a typed `Operation`; every operation produces a `Revision`;
every revision is immutable. The Rust core persists to SQLite inside a
single `.aidoc` ZIP package, and an MCP server exposes the same operation
engine to AI agents.

## Quickstart

```sh
# Build everything
cargo build --workspace

# Run the demo (create → update → revert, dump HTML)
cargo run -p aidoc-cli -- demo examples/order-system.aidoc \
    --export-html examples/order-system.html

# Scaffold one of the bundled templates
cargo run -p aidoc-cli -- example order-system examples/order-system.aidoc
cargo run -p aidoc-cli -- example api-system    examples/api-system.aidoc
cargo run -p aidoc-cli -- example knowledge-graph examples/knowledge-graph.aidoc

# Start the MCP server (line-delimited JSON-RPC over stdio)
cargo run -p aidoc-mcp
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

## Architecture

```text
React/Tiptap Editor  (apps/desktop)
         │
         ▼
    AIDoc API
         │
         ▼
Rust Core  (crates/aidoc-model / -storage / -operation / -history ...)
         │
         ▼
      SQLite
         │
         ▼
  ZIP Package (.aidoc)

AI / Agent → AIDoc Core (Operation → Validator → SQLite → Revision)
```

## Layout

```
aidoc/
├── apps/
│   ├── desktop/     Tauri 2 + React + Tiptap editor
│   └── cli/         aidoc CLI binary
├── crates/
│   ├── aidoc-model/      Document / Node / Relation / Revision / Operation / Provenance
│   ├── aidoc-storage/    SQLite schema + transaction wrapper
│   ├── aidoc-operation/  Operation engine + optimistic concurrency
│   ├── aidoc-history/    Revert (always produces a new revision)
│   ├── aidoc-validator/  Identity / Structure / Relation / Revision / CodeRef
│   ├── aidoc-package/    ZIP .aidoc reader/writer + manifest + temp workspace
│   ├── aidoc-renderer/   AIDoc → HTML (diagram / code-ref / requirement extensions)
│   ├── aidoc-exporter/   HTML / Markdown export
│   ├── aidoc-mcp/        stdio MCP server (15 tools, see below)
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

Tools:

```
init_aidoc  open_aidoc   save_aidoc   list_nodes   show_node
create_node  update_node  delete_node  apply_operation
history     revert       export_html  validate
branch      merge
```

Wire handshake: send `initialize` → receive server info → send
`notifications/initialized` → `tools/list` / `tools/call …`.

## CI

`.github/workflows/ci.yml` runs three jobs on every PR:

1. `rust` matrix (Ubuntu + Windows): `cargo fmt --check`, `cargo clippy -D warnings`,
   `cargo test --workspace`, plus release builds of `aidoc-cli` and `aidoc-mcp`.
2. `ui`: `npm run build` for the React + Tiptap frontend.
3. `integration-smoke`: `cargo test -p aidoc-mcp --test stdio_smoke` end-to-end
   handshake over stdio.