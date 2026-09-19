# Changelog

All notable changes to AIDoc, newest first.

## v0.1.1 — 2026-09-19

Desktop app maturity pass. Closes every remaining gap on the
`docs/TASKS.md` plan.

### Desktop (Tauri 2)

**Backend (Rust)**

- Resolved 24 Tauri commands wired end-to-end through the UI; added two new
  commands: `save_doc_as`, `abort_ai_chat`, plus `list_documents` /
  `close_doc` for the tab manager.
- New helper `resolve_mcp_bin()` locates the `aidoc-mcp` binary via
  `AIDOC_MCP_BIN` env var → walk-up from `current_exe()` → CWD fallback;
  now portable across Windows / macOS / Linux without per-platform patches.
- `ai_chat_impl` rewritten to spawn a Python subprocess with piped stdio,
  read stdout line-by-line in a worker thread, and emit
  `ai-chunk` / `ai-done` / `ai-error` / `ai-stderr` / `ai-usage` events for
  streaming. `abort_ai_chat` keeps a handle to the child and `kill()`s it
  on demand.
- `parse_kind` extended to all 22 NodeKind variants
  (ListItem / Table / TableRow / TableCell / Link / Image / Details / Summary).
- CSP tightened in `tauri.conf.json` — `script-src` no longer permits
  `unsafe-inline`; `style-src` kept it (React + mermaid emit inline style).
- Bundled `tauri-plugin-updater` + `tauri-plugin-dialog` (already present);
  updater config wired (GitHub releases endpoint placeholder pending CI
  release workflow).

**Frontend (React + Tiptap + mermaid)**

- New components: `BranchDialog`, `LinkDialog`, `AttributesDialog`,
  `HelpDialog`, `CodeRefEditor`, `ImageEditor`, plus `NodeTree` for the
  recursive parent → children sidebar.
- Recent-files + auto-restore on launch (`useRecentFiles` hook backed by
  `localStorage`); tab-style chips on the Welcome page with one-click
  remove.
- Save As wired through Header `DropdownMenu` (Save / Save As…).
- Nested / tree-view sidebar with collapse toggle and drag-to-reparent
  (drop on a node nests the dragged node under it).
- `KindStrip` covers all 22 kinds; missing kinds (list-item / table /
  link / image / details / summary) are now creatable.
- AI chat panel: streaming chunks, Stop button, persistent history saved
  into the document's `__ai_history__` node's `attributes.history`
  (debounced 400ms), live token-usage counter (parsed from
  `__USAGE__:` line emitted by `apps/ai/agent.py`).
- Code-ref editor: source path + line inputs with copy-to-clipboard.
- Image editor: URL paste or file picker → base64 inline (5 MiB cap).
- ⌘F focuses the sidebar search; ⌘/ opens the Help dialog listing every
  shortcut. Header gains a Keyboard icon for the Help dialog.
- i18n with `en.json` + `zh-CN.json` catalogs, auto-detected locale,
  manual switch in Settings.
- AI provider switcher in Settings (OpenAI / Ollama / Custom URL), each
  with sensible defaults for `base_url` and `model`.
- Header tab chip showing the current document with a close button
  returning to the Welcome page.
- macOS / Linux bundle config (`tauri.conf.json`); PNG icons generated
  from the existing `.ico` source for Linux releases.

### Tests

- 10 / 10 Rust unit tests pass (`cargo test -p aidoc-desktop`) covering
  `parse_kind` (all 22 kinds + reject), `resolve_mcp_bin` env precedence,
  `build_op` error surface, `node_to_dto` (kebab-case + attributes),
  `seed_root` / `seed_initial_revision`, `read_info` shape, and the CSP
  invariant test that reads `tauri.conf.json` directly.
- 31 / 31 JS unit tests (`pnpm test` via `node --test`) covering
  `useRecentFiles.mergeRecent` / `parseStored`, `editorSerialize.wrap` /
  `unwrap` (round-trip for code / link / image / details / summary /
  paragraph), and `nodeTreeIndex.buildTreeIndex` (parent lookups,
  unknown-parent fallback, position sort, empty input).
- `pnpm test` script added; `tsc -b --noEmit` and `vite build` both
  pass in 12s.

### Tooling

- `docs/TASKS.md` documents every stage as ✅; the file is the
  hand-off guide for the next session.
- GitHub Actions `.github/workflows/ci.yml` extended with explicit
  desktop `cargo test -p aidoc-desktop`, `pnpm exec tsc`,
  `pnpm test`, and `cargo check -p aidoc-desktop` steps.

### Known follow-ups (out of scope)

- Vitest replaced by `node --test` because the npm registry mirror was
  unreachable during the dev window; bring back Vitest + jsdom once
  connectivity is restored.
- Real on-disk `.aidoc/assets/` storage for images — currently inline
  base64. Needs a schema decision before implementation.
- True multi-session state (`HashMap<DocId, SessionHandle>`) — the UI is
  shaped for tabs but the backend still holds one session at a time.

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