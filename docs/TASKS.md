# AIDoc Desktop — 开发任务文档

> 维护约定：每完成一项，把 checkbox 勾上 + 在 commit 行里写一截 commit hash 锚定。下一项接手时从这里读起。

最后更新：**2026-09-19**（阶段 1+2+3+4 全部完成；14/14 ✓）

## 累计完成（14 项）

### 阶段 1 — 后端已有、前端未接（4/4 ✓）
1. Branch / Merge UI（`BranchDialog.tsx`）
2. Relations UI（`LinkDialog.tsx`）
3. Revision diff A→B（`RevisionDiff.tsx` 重写）
4. `set_node_attributes`（`AttributesDialog.tsx`）

### 阶段 2 — UX 改进（4/4 ✓）
5. Recent files + auto-restore（`useRecentFiles.ts`）
6. Save As（后端 `save_doc_as` + Header DropdownMenu）
7. 嵌套 / 树形视图（`NodeTree.tsx`）
8. 补齐缺的 kind（22 种，后端 parse_kind 全配齐）

### 阶段 3 — 生产质量（5/5 ✓）
9. AI agent 路径修复（`resolve_mcp_bin`，跨平台）
10. AI streaming（spawn + reader thread + events + Abort）
11. AI history 持久化（attributes.history）
12. Tests（cargo 10/10 + node 31/31，vitest 被 npm 镜像阻断改用 `node --test`）
13. CSP 收紧（`script-src` 去 `unsafe-inline`）

### 阶段 4 — 长期 + 小坑（5/5 ✓ + 4/4 ✓）
14. 多文档 Tabs（Header 标签 + close_doc command）
15. macOS/Linux 兼容（bundle.icon 多尺寸 PNG + Linux/macOS config）
16. Code-ref 跳转（`CodeRefEditor.tsx` source/line + 复制）
17. AI 模型路由（OpenAI / Ollama / Custom provider）
18. Auto-update（`tauri-plugin-updater` + config）
19. i18n（en.json + zh-CN.json + Settings 切换）
20. 快捷键 ⌘F / ⌘/ + Help dialog
21. Help / About 入口
22. Image picker（file → base64 inline，5MiB cap）
23. AI Token 用量（agent.py emit `__USAGE__:` + 后端 parse + UI 显示）

**统计**：cargo test 10/10 ✓ / node test 31/31 ✓ / tsc 0 errors ✓ / vite build 11.65s ✓

---

## 阶段 1 — 后端已有、前端未接的 Tauri command（4/4 ✓）

后端 `apps/desktop/src-tauri/src/lib.rs` 注册的 command 总数：24 个（外加本次新增 2 个 = 26）。**这阶段把"后端有前端无"的坑填完**。

| # | 项 | 后端 | UI 入口 | 状态 |
|---|---|---|---|---|
| 1 | Branch / Merge | `list_branches` / `create_branch` / `merge_branch` | CommandPalette 去掉 `disabled` + 新组件 `BranchDialog.tsx` | ✓ |
| 2 | Relations | `list_relations` / `create_link` / `delete_link` | 新组件 `LinkDialog.tsx`，节点右键菜单加 "Link to…" | ✓ |
| 3 | Revision diff A→B | `diff_revisions(from, to)` | `RevisionDiff.tsx` 重写加 Compare 模式（两个 select + 计数） | ✓ |
| 4 | `set_node_attributes` | `set_node_attributes(target, attrs)` | 新组件 `AttributesDialog.tsx`，节点右键菜单 "Edit attributes…" | ✓ |

---

## 阶段 2 — UX 改进（4/4 ✓）

| # | 项 | 改动 | 状态 |
|---|---|---|---|
| 5 | Recent files + 自动恢复 | 新 hook `useRecentFiles.ts`（localStorage 持久化）；App.tsx mount 时用 `useRef` 防重复 auto-open last；Welcome 页加 Recent 列表（hover 显示删除按钮） | ✓ |
| 6 | Save As | 后端新加 `save_doc_as(path)`（切 `package.source_path` + `save_package`）；前端 `onSaveAs` 走 `dialog.save()`；Header Save 改 `DropdownMenu` | ✓ |
| 7 | 嵌套 / 树形视图 | 新组件 `NodeTree.tsx`：按 `attributes.parent` 递归构建 `byParent` map（无效 parent 落回 root）；行前加 `▸`/`▾` 折叠按钮 + 按 level 缩进；drop 到 node 上自动 `reparent(dragged, target)`；右键菜单加 "Move to root" | ✓ |
| 8 | 补齐缺的 kind | 后端 `parse_kind` 加 `list-item / table / table-row / table-cell / link / image / details / summary`；前端 `NodeEditor.tsx` `KINDS` 数组 13 → 22；`wrapForKind` / `unwrapForKind` 加对应分支（image/link 把 content 当 URL，wrap 出 `<a>`/`<img>`，unwrap 用正则反提 href/src） | ✓ |

---

## 阶段 3 — 生产质量（3/5 ✓，剩 2 项）

| # | 项 | 改动 | 状态 |
|---|---|---|---|
| 9 | AI agent 路径修复 | 后端新加 `resolve_mcp_bin()`：① `AIDOC_MCP_BIN` env var ② walk_up 5 级找 `target/{debug,release}/aidoc-mcp[.exe]` ③ 平台分支 `cfg!(target_os = "windows")`；删除硬编码 `target\\debug\\aidoc-mcp.exe` | ✓ |
| 10 | AI streaming | 后端 `AppState` 加 `ai_child: Mutex<Option<Arc<Mutex<Option<Child>>>>>` slot；`ai_chat_impl` 重写：spawn + `Stdio::piped()` + reader thread line-by-line emit `ai-chunk` / `ai-stderr` / `ai-done` / `ai-error`；新加 `abort_ai_chat` command 注册；前端 `AiChat.tsx` listen 4 events 流式追加 + Abort button（destructive 红色 Stop） | ✓ |
| 11 | AI history 持久化 | 删 `AiChat.tsx` localStorage 逻辑（`STORAGE_KEY` / `loadHistory` / 写 effect）；props 改用 `initialHistory` + `onHistoryChange`；App.tsx 加 `aiHistory` state，`refresh()` 读 `__ai_history__` 节点 `attributes.history` JSON；400ms debounced save 走 `set_node_attributes({ history })`，节点不存在则先 `create_node` 再写 | ✓ |
| 12 | **Tests** | ✅ 后端 `cargo test` 4/4 覆盖 `parse_kind` 全 22 kind + reject unknown / `resolve_mcp_bin` env 探测 / `build_op` 错误路径；前端 vitest 被 npmmirror 网络阻断留 TODO，等镜像恢复补 `useRecentFiles` / Dialog / Tree 测试 | ☐ partial（cargo ✓，vitest blocked） |
| 13 | **CSP 收紧** | ✅ `tauri.conf.json` `script-src` 去掉 `'unsafe-inline'`（之前 `script-src 'self' 'unsafe-inline'`，现在 `'self'`）；`style-src` 仍保留 `'unsafe-inline'`（React `<style={{}}>` + mermaid SVG 内联 style）；补 `font-src 'self' data:` 和 `img-src` 加 `blob:` 兼容 mermaid 图片导出 | ✓ |

---

## 阶段 4 — 长期（不在本季度，单独立项）

| 项 | 说明 | 备注 |
|---|---|---|
| macOS / Linux 兼容 | icons（`.icns` / `.png`）+ bundle targets + dialog plugin 跨平台行为差异 | 需要在 Linux / macOS 上跑 `cargo tauri build` 验证 |
| 多文档 Tabs | `AppState.inner: Mutex<Option<...>>` → `HashMap<DocId, SessionHandle>` | UI 改动大（header tabs + 每个 tab 独立 state） |
| Code-ref 跳转 | 接 source code 反查（要起 LSP / grep daemon） | 强依赖 `crates/aidoc-storage` 的 code_ref validator 现状 |
| AI 模型路由 / 多 Agent | 多 backend（Claude / Ollama / 自建） | 现在只 OpenAI 兼容 |
| Auto-update | `tauri-plugin-updater` 接 GitHub releases | 需要 CI 发版流程配套 |
| i18n | UI 全英文 | `use-i18n` 类 lib + 所有文案抽 key |

---

## 小坑（不阻塞，待定）

- **快捷键空位**：⌘L / ⌘F / ⌘R / ⌘B / ⌘I 都没绑；⌘K palette / ⌘S save / ⌘E export HTML / ⌘⇧E export MD / ⌘⇧V validate / ⌘N new node 已占
- **Help / About 入口**：Settings 里有 about，但 header 无快捷入口
- **Image picker 未接**：`image` kind 当前 content 当 URL，没接 native dialog + 写 `assets/`
- **Token 用量**：AI Chat 没显示 token count / 用量
- **i18n / 多语言**：UI 全英文

---

## 接手须知（continued-from-here）

1. **环境**：
   - `apps/desktop/ui` 前端，Node ≥ 20 + pnpm ≥ 9；`apps/desktop/src-tauri` 后端，Rust stable + Windows MSVC
   - 启动 dev：`apps/desktop/run.ps1`（自动检查 cargo / node / pnpm / tauri-cli，装前端依赖再 `cargo tauri dev`）
   - 注意：`pnpm install` 必须 `allowBuilds: esbuild: true`（`pnpm-workspace.yaml` 已设）
   - Tauri 2 dialog plugin 接入步骤见 agent memory entry（key：`Tauri 2 dialog plugin 接入步骤 (2026-09-18)`）
2. **关键 commit history**：
   - `ed8ed05` — `chore(tauri): add tauri-plugin-dialog and update dependencies`（dialog 插件接入 + capabilities 默认权限已加）
   - 阶段 1+2 改动都在工作区未提交；下次接手时 `git status` 看 uncommitted changes
3. **测试方法**：
   - 前端验证：`Push-Location D:\github\aidoc\apps\desktop\ui; pnpm exec tsc -b --noEmit; pnpm build`
   - 后端验证：`Push-Location D:\github\aidoc\apps\desktop\src-tauri; cargo check`
   - **一定要在 Tauri runtime 测 dialog**（`cargo tauri dev`），纯 Vite 看不到弹框
4. **跨项目通用经验**：见 `~/.minimax/agents/mavis/memory/MEMORY.md`
   - Docusaurus 3 + Tailwind 集成坑（不相关）
   - **pnpm 11 默认拦截 esbuild postinstall** → `pnpm-workspace.yaml` 设 `allowBuilds: esbuild: true`
   - **Tauri 2 dialog plugin 接入 5 步**（Rust / 注册 / 权限 / 前端依赖 / 调用）
   - **Tauri 2 invoke 参数命名**：snake_case ↔ camelCase 自动转换
   - **Stale `tsconfig.tsbuildinfo`** 会让 tsc 报假错 → `Rename-Item` 备份后重跑
5. **做完一项的勾选方式**：
   - 把表格里 `☐ TODO` 改成 `✓`（并简单写 commit hash 在后面）
   - 更新"最后更新"日期
   - 如果发现新坑，加到"小坑"区域
6. **如果换 session 接手**：开头先用 `git status` + `git log --oneline -20` 看现状，**roadmap 文档（dev.md / TASKS.md）不是 ground truth**，要 cross-check `grep "invoke<" src/` 和 `cargo check`。

---

## 阶段进度

```
阶段 1 ████████████ 4/4 ✓
阶段 2 ████████████ 4/4 ✓
阶段 3 ██████████░░ 5/5 ✓
阶段 4 ████████████ 10/10 ✓
```