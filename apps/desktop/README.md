# AIDoc Desktop (Tauri 2 + React)

Tauri 2 shell wrapping the AIDoc Rust core. React + Vite UI calls Tauri commands
that in turn drive `aidoc::operation::apply_operation`, `aidoc::history::revert_to`,
and the SQLite-backed `aidoc-storage`.

## 一次性环境

- Rust stable + Windows MSVC toolchain (`rustup default stable-x86_64-pc-windows-msvc`)
- Microsoft Visual Studio C++ Build Tools (for linking)
- WebView2 Runtime（Win11 自带，Win10 需 [Evergreen installer](https://developer.microsoft.com/microsoft-edge/webview2/)）
- Node ≥ 20, pnpm ≥ 9
- `cargo install tauri-cli --version "^2.0"`（**慢**，建议先做这一步后台跑）

## 启动 dev

```sh
# 前端 deps
cd apps/desktop/ui
pnpm install

# Rust + Tauri（需先 `cargo install tauri-cli --version "^2.0"`）
cd ../src-tauri
cargo tauri dev        # 启 Tauri，会编 src-tauri + 跑 vite dev
```

第一次 dev 编译 `src-tauri/` 要几分钟。

## 打包

```sh
cd apps/desktop/src-tauri
cargo tauri build
# 产物：target/release/bundle/{msi,nsis,exe}/...
```

## Tauri 命令 → aidoc core 映射

| Tauri command       | aidoc 调用                                    |
|---------------------|-----------------------------------------------|
| `init_doc`          | `aidoc::create_package` + seed root / R000    |
| `open_doc`          | `aidoc::open_package`                         |
| `save_doc`          | `aidoc::save_package`                         |
| `list_nodes`        | `aidoc_storage::crud::list_nodes`             |
| `list_revisions`    | `aidoc_storage::crud::list_revisions`         |
| `update_node`       | `aidoc::apply_operation(Update)`              |
| `revert`            | `aidoc::revert_to`                            |
| `export_html`       | `aidoc::exporter::export_html`                |

UI ↔ Core 边界严格遵守 spec §10：React 只通过 `invoke()` 调 command，不直接碰 SQLite。