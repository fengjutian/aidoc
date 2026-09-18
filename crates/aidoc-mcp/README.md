# aidoc-mcp

MCP (Model Context Protocol) server over stdio that exposes the AIDoc
operation engine as tools. Connect Claude / Cursor / VS Code to it and the
AI can drive `.aidoc` files through the same `apply_operation` /
`revert_to` pipeline the CLI uses.

## Build

```sh
cargo build --release -p aidoc-mcp
# binary: target/release/aidoc-mcp.exe
```

## Tool surface

| Tool             | Args                                                 |
|------------------|------------------------------------------------------|
| `init_aidoc`     | `path`, `doc_id`, `title`                             |
| `open_aidoc`     | `path`                                                |
| `save_aidoc`     | -                                                     |
| `list_nodes`     | -                                                     |
| `show_node`      | `node_id`                                             |
| `create_node`    | `target`, `content`                                   |
| `update_node`    | `target`, `content`                                   |
| `delete_node`    | `target`                                              |
| `apply_operation`| `op_json` (full Operation JSON)                       |
| `history`        | -                                                     |
| `revert`         | `target_revision` (e.g. `R101`)                       |
| `export_html`    | -                                                     |
| `validate`       | -                                                     |

Every write goes through `apply_operation` so every change is a new
revision (MUST 3). `revert` always creates a new revision too (MUST 4).

## Wire it up

### Claude Desktop

Edit `claude_desktop_config.json` (on Windows: `%APPDATA%\Claude\`):

```json
{
  "mcpServers": {
    "aidoc": {
      "command": "D:/github/aidoc/target/release/aidoc-mcp.exe",
      "args": [],
      "env": {}
    }
  }
}
```

Restart Claude Desktop. The AI now sees 13 `aidoc__*` tools.

### Cursor

`.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "aidoc": {
      "command": "D:/github/aidoc/target/release/aidoc-mcp.exe",
      "args": []
    }
  }
}
```

### VS Code (Continue / Cline / etc.)

Same shape as Cursor. The tool names are exactly the strings above.

## Workflow example

```
AI: I'll start by initializing a doc.
  → init_aidoc { path: "examples/demo.aidoc", doc_id: "demo", title: "Demo" }

AI: Now create the architecture section.
  → create_node { target: "architecture", content: "系统采用微服务架构。" }

AI: Update with more detail.
  → update_node { target: "architecture", content: "..." }

AI: Revert that last edit.
  → revert { target_revision: "R002" }

AI: Export to HTML for review.
  → export_html → writes to a new tab via stdio.
```

## Architecture

```
Claude / Cursor
        │
        ▼ JSON-RPC over stdio
   aidoc-mcp (rmcp)
        │
        ▼
   aidoc crate
   ├── apply_operation  ──→ SQLite store (transactional)
   ├── revert_to        ──→ SQLite store (transactional)
   ├── validator        ──→ reads SQLite
   └── exporter         ──→ reads SQLite
```

Nothing touches the DB except the `aidoc` crate. The MCP layer is pure
serialization + dispatch.

## Schema for empty args

The server registers each tool with an empty JSON Schema
(`{"type":"object"}`) — clients should pass arguments as a flat JSON
object whose keys match the table above. No `Parameters<T>` wrapping.