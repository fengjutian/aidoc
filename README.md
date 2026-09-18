# AIDoc

AI-native structured document format. Implementation of the [AIDoc v0.1 Format Specification](./docs/AIDoc%20v0.1%20Format%20Specification.md).

## 架构

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

## 目录

```
aidoc/
├── apps/
│   ├── desktop/     Tauri 2 + React + Tiptap (P1)
│   └── cli/         aidoc CLI binary
├── crates/
│   ├── aidoc-model/      Document / Node / Relation / Revision / Operation / Provenance
│   ├── aidoc-storage/    SQLite schema + transaction
│   ├── aidoc-operation/  Operation engine + conflict detection
│   ├── aidoc-history/    Revision / Change / Revert
│   ├── aidoc-validator/  Identity / Structure / Relation / Revision / CodeRef
│   ├── aidoc-package/    ZIP .aidoc 读写 + manifest + workspace
│   ├── aidoc-renderer/   AIDoc → HTML（含 diagram/code-ref/requirement 扩展）
│   ├── aidoc-exporter/   HTML / Markdown 导出
│   └── aidoc/            Facade：re-export 所有子 crate
├── examples/        示例 .aidoc
└── docs/            规范
```

## 4 条 MUST

1. 节点必须有稳定 ID
2. 修改必须通过 Operation
3. 每次修改必须产生 Revision
4. Revert 不能删除历史

## 编译

```sh
cargo build --workspace
cargo test  --workspace
cargo run -p aidoc-cli -- --help
cargo run -p aidoc-cli -- demo examples/order-system.aidoc
```