# Changelog

All notable changes to AIDoc, newest first.

## v0.1 — 2026-09-18

First user-facing release of the v0.1 implementation. Matches
`docs/AIDoc v0.1 Format Specification.md` end-to-end.

### CLI (14 subcommands)

- `init` / `info` — empty document lifecycle.
- `node-list` / `node-show` — inspect nodes by id.
- `apply` — apply an Operation JSON (file or stdin), with optional
  `--print-revision` to surface the new revision id.
- `history` — print revision history; supports `--json` and `--branch=<name>`.
- `diff` — per-revision snapshot diff (add / remove / change / unchanged);
  `from` and `to` are both optional, `--branch=<name>` scopes both to one branch.
- `revert` — always produces a new revision (MUST 4).
- `validate` — runs all 5 validators; exits non-zero on errors.
- `export` — HTML or Markdown (`--format html|md`).
- `demo` — full create → update → revert loop with optional export.
- `example` — scaffold one of `order-system` / `api-system` / `knowledge-graph`.
- `branch <name>` — tag the current head with a named branch (spec §34).
- `merge <branch>` — collapse a named branch back into main (spec §35, simplified).

### MCP server (`aidoc-mcp`)

15 tools exposed over line-delimited JSON-RPC over stdio (rmcp 0.3.2):

```
init_aidoc   open_aidoc   save_aidoc
list_nodes   show_node    create_node    update_node   delete_node
apply_operation
history      revert       export_html    validate
branch       merge
```

End-to-end smoke test (`crates/aidoc-mcp/tests/stdio_smoke.rs`) spawns the
real binary and verifies `initialize` → `tools/list` → `init_aidoc` →
`list_nodes` → `branch` → `merge`.

### Core (Rust workspace)

9 crates:

- `aidoc-model` — Document / Node / Relation / Revision / Operation / Provenance.
- `aidoc-storage` — SQLite schema + transaction wrapper.
- `aidoc-operation` — Operation engine + optimistic concurrency.
- `aidoc-history` — Revert (always produces a new revision).
- `aidoc-validator` — Identity / Structure / Relation / Revision / CodeRef.
- `aidoc-package` — ZIP `.aidoc` reader/writer + manifest + temp workspace.
- `aidoc-renderer` — AIDoc → HTML.
- `aidoc-exporter` — HTML / Markdown export.
- `aidoc` — Facade re-exporting everything.
- `aidoc-mcp` — stdio MCP server binary.

### Apps

- `apps/desktop` — Tauri 2 + React + Tiptap editor.
- `apps/cli` — `aidoc` CLI binary.

### Spec compliance

All 4 MUST rules from spec §54:

1. Stable node IDs.
2. Mutations only via `Operation`.
3. Every operation produces a `Revision`.
4. Revert creates a new revision (history is immutable).

Plus spec §29 AI Provenance: `prompt` / `tool_calls` / `temperature` /
`input_refs` / `output` / `reasoning_summary` are all carried on the
`Provenance::Operation` variant, kept out of HTML body per spec §29.

Spec §34 Branch and §35 Merge (foundation): storage supports named
branches and multi-parent revisions via side-tables (`revision_branches`,
`revision_parents`); the apply engine recognises `OperationType::Branch`.

### CI

`.github/workflows/ci.yml` runs on every PR:

1. `rust` matrix (Ubuntu + Windows): `cargo fmt --check`, `cargo clippy -D warnings`,
   `cargo test --workspace`, release builds of `aidoc-cli` and `aidoc-mcp`.
2. `ui`: `npm run build` for the React + Tiptap frontend.
3. `integration-smoke`: `cargo test -p aidoc-mcp --test stdio_smoke` end-to-end
   handshake over stdio.

### Test coverage

- 19 unit / integration tests across the workspace.
- All 5 validator categories verified to fire on deliberately-broken fixtures
  (`crates/aidoc-validator/tests/trigger_all.rs`).
- MCP stdio smoke test (1 test, 6 round-tripped JSON-RPC messages).
- Per-revision snapshot diff covered via `aidoc diff` and the `from/to` defaults.

### Schema notes

- `revision_branches` and `revision_parents` are side-tables; existing
  documents don't need migration to gain branch / multi-parent support — they
  just have no rows in those tables until a `Branch` op runs.

### Known gaps (deferred per spec)

- Tauri desktop build was not run end-to-end on the development host
  (no WebView2 / MSVC tools). The Rust shell + UI both compile.
- Spec §34-§35 describe true conflict detection (`NODE_CONFLICT`,
  `CONTENT_CONFLICT`, `STRUCTURE_CONFLICT`). v0.1 records the merge
  revision but does not yet run auto-resolution; the data model supports it.