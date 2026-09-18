如果是按照刚才的 **AIDoc v0.1** 来开发，我建议不要一开始做成“纯 Web 编辑器”，而是做成一个**本地优先的 AIDoc Runtime + Editor**。

结合你的技术栈，我会推荐：

## 1. 总体技术栈

```text
┌─────────────────────────────────────────┐
│             AIDoc Desktop               │
│             Tauri 2 + React             │
├─────────────────────────────────────────┤
│ React + TypeScript                      │
│ ├── Document Editor                     │
│ ├── Tree / Outline                      │
│ ├── AI Chat                             │
│ ├── Revision / Diff                     │
│ ├── Mermaid                             │
│ └── RAG UI                              │
├─────────────────────────────────────────┤
│             AIDoc Core                  │
│              Rust                       │
│ ├── Package (.aidoc)                    │
│ ├── SQLite                               │
│ ├── Node / Relation                      │
│ ├── Revision / Operation                 │
│ ├── Patch / Diff                         │
│ ├── Validation                           │
│ └── Import / Export                      │
├─────────────────────────────────────────┤
│ Optional AI / RAG                       │
│ Python                                   │
│ ├── LLM                                  │
│ ├── Embedding                            │
│ ├── RAG                                  │
│ └── Code Analysis                        │
└─────────────────────────────────────────┘
```

### 我的核心推荐

| 层         | 技术                           |   推荐度 |
| --------- | ---------------------------- | ----: |
| Desktop   | **Tauri 2**                  | ⭐⭐⭐⭐⭐ |
| UI        | **React + TypeScript**       | ⭐⭐⭐⭐⭐ |
| Editor    | **Tiptap / ProseMirror**     | ⭐⭐⭐⭐⭐ |
| Core      | **Rust**                     | ⭐⭐⭐⭐⭐ |
| Database  | **SQLite**                   | ⭐⭐⭐⭐⭐ |
| Package   | **ZIP**                      | ⭐⭐⭐⭐⭐ |
| HTML      | **HTML5**                    | ⭐⭐⭐⭐⭐ |
| Diagram   | **Mermaid**                  | ⭐⭐⭐⭐⭐ |
| Diff      | **Rust diff / custom patch** | ⭐⭐⭐⭐⭐ |
| AI        | OpenAI-compatible API        | ⭐⭐⭐⭐⭐ |
| RAG       | Python + LlamaIndex          |  ⭐⭐⭐⭐ |
| Vector DB | SQLite → Qdrant              |  ⭐⭐⭐⭐ |
| Search    | SQLite FTS5                  | ⭐⭐⭐⭐⭐ |

---

# 2. 最重要：Rust 做 AIDoc Core

这是我最建议你改变的地方。

不要：

```text
React
 ↓
直接操作 HTML
 ↓
SQLite
```

而是：

```text
React
 ↓
AIDoc API
 ↓
Rust Core
 ↓
SQLite
```

Rust Core 才是整个 AIDoc 的真正核心。

例如：

```rust
pub struct AIDoc {
    pub document: Document,
    pub nodes: NodeStore,
    pub relations: RelationStore,
    pub revisions: RevisionStore,
    pub operations: OperationStore,
}
```

然后提供：

```rust
create_node()
update_node()
delete_node()
move_node()

create_relation()
delete_relation()

apply_operation()
revert_revision()

create_snapshot()
restore_snapshot()

validate()
export_html()
```

这样以后你可以：

```text
Tauri
CLI
VS Code Extension
Server
Web Editor
AI Agent
```

全部使用同一个 Core。

---

# 3. SQLite 不要让 React 直接碰

建议：

```text
React
  ↓
Tauri Command
  ↓
Rust
  ↓
SQLite
```

而不是：

```text
React
  ↓
SQLite
```

原因很简单：

**事务、Revision、Operation、Conflict 都属于 Core 的职责。**

例如：

```text
updateNode()
```

内部实际执行：

```text
BEGIN

检查 expected_revision

更新 Node

创建 Change

创建 Operation

创建 Revision

更新 current_revision

COMMIT
```

前端只知道：

```typescript
await aidoc.updateNode(...)
```

---

# 4. Editor：推荐 Tiptap

如果要做真正的 AIDoc 编辑器，我会优先选择：

**Tiptap + ProseMirror**

而不是直接：

```text
contenteditable
```

也不是自己实现整个编辑器。

原因是 AIDoc 本质上需要：

```text
Document
 ├── Section
 ├── Paragraph
 ├── Code
 ├── Table
 ├── Link
 ├── Diagram
 ├── Requirement
 ├── Decision
 └── CodeRef
```

Tiptap/ProseMirror 本身就是结构化文档模型。

可以定义：

```text
AIDoc Node
```

例如：

```text
paragraph
heading
section
codeBlock
diagram
codeRef
requirement
decision
```

---

# 5. 但是不要让 Tiptap 成为 AIDoc Core

这一点非常重要。

不要设计成：

```text
Tiptap JSON
     ↓
SQLite
```

因为这样最终会变成：

> AIDoc = Tiptap 文档格式。

这是不应该的。

应该是：

```text
              AIDoc Model
                   │
          ┌────────┴────────┐
          ↓                 ↓
     Tiptap View        HTML Renderer
          │
          ↓
      Editor UI
```

Tiptap 是**编辑器实现**。

AIDoc Model 才是**格式标准**。

---

# 6. HTML Renderer

建议自己实现一个：

```text
AIDoc → HTML
```

Renderer。

例如：

```rust
render_html(document)
```

输入：

```text
Node
 ├── Section
 ├── Paragraph
 ├── CodeRef
 └── Diagram
```

输出：

```html
<section id="database">
    <h2>数据库</h2>

    <code-ref
        id="order-service"
        file="src/order/order.service.ts"
        symbol="OrderService">
    </code-ref>
</section>
```

这样 AIDoc 可以完全脱离 Tiptap。

---

# 7. Mermaid

Diagram 第一版直接：

```text
Mermaid
```

不要自己开发流程图引擎。

AIDoc：

```html
<diagram
    id="order-flow"
    type="flowchart"
    engine="mermaid">
```

内部：

```text
diagram.source
      ↓
Mermaid
      ↓
SVG
```

以后再扩展：

```text
Mermaid
PlantUML
Graphviz
Excalidraw
D2
```

---

# 8. ZIP + SQLite

这一部分非常适合 Rust。

例如：

```text
.aidoc
```

本质：

```text
ZIP
├── manifest.json
├── document/
│   └── document.html
├── .internal/
│   └── document.db
└── assets/
```

Rust 负责：

```text
Open
 ↓
Unzip
 ↓
SQLite
 ↓
Edit
 ↓
Commit
 ↓
Pack
```

可以做：

```rust
AidocPackage::open(path)
AidocPackage::save()
AidocPackage::extract()
AidocPackage::pack()
```

---

# 9. 一个值得考虑的优化：工作目录

不要每修改一次就：

```text
解压 ZIP
修改 SQLite
重新 ZIP
```

这样效率不好。

可以采用：

```text
Open example.aidoc
        ↓
Temporary Workspace
        ↓
SQLite + Assets
        ↓
编辑
        ↓
Save
        ↓
Pack
        ↓
example.aidoc
```

也就是：

```text
.aidoc
  ↓
Workspace
  ↓
SQLite
```

保存时：

```text
Workspace
  ↓
ZIP
  ↓
.aidoc
```

---

# 10. AI 层

AI 不应该直接操作 SQLite。

错误：

```text
LLM
 ↓
SQL
 ↓
SQLite
```

应该：

```text
LLM
 ↓
AIDoc Operation
 ↓
Validator
 ↓
AIDoc Core
 ↓
SQLite
```

例如 AI 输出：

```json
{
  "operation": "update",
  "target": "database",
  "expected_revision": "R103",
  "patch": {
    "content": "MySQL 8.4"
  }
}
```

然后 Rust：

```text
validate
 ↓
permission
 ↓
conflict
 ↓
transaction
 ↓
revision
```

这会让 AI 变成：

> AIDoc 的一种操作客户端

而不是：

> AIDoc 的数据库管理员。

---

# 11. AI Agent

以后可以做：

```text
AIDoc Agent
```

例如：

```text
用户：
更新订单架构文档

        ↓

Agent

        ↓

读取 AIDoc Graph

        ↓

读取 CodeRef

        ↓

读取 Git

        ↓

分析代码

        ↓

生成 Operations

        ↓

AIDoc Core

        ↓

Revision
```

这里非常适合你之前的 Agent / MCP 思路。

---

# 12. Python 放在哪里？

我建议：

**不要让 Python 成为 AIDoc 核心。**

Python 用于：

```text
AI
RAG
Embedding
Code Analysis
Document Processing
```

例如：

```text
Rust Core
    │
    ├── SQLite
    ├── Revision
    ├── Operation
    └── Package
           │
           ↓
        Python
           │
       ┌───┴────┐
       ↓        ↓
      LLM      RAG
```

这样即使以后不用 Python：

```text
AIDoc Core
```

仍然可以独立运行。

---

# 13. RAG 技术

第一版不要上 Qdrant。

直接：

```text
SQLite
+
FTS5
```

例如：

```text
nodes
node_embeddings
node_fts
```

实现：

```text
关键词搜索
+
结构搜索
+
关系搜索
```

等数据规模真的达到：

```text
百万级 Node
```

再：

```text
SQLite
     +
Qdrant
```

---

# 14. Git 集成

AIDoc 非常适合 Git。

例如：

```text
project.aidoc
       │
       ├── document
       ├── history
       └── assets
```

但是**不要直接依赖 Git 实现 AIDoc History**。

应该：

```text
AIDoc Revision
      +
Git Commit
```

两套系统各自负责不同事情。

```text
Git
 ↓
文件级版本

AIDoc
 ↓
节点级版本
```

以后可以关联：

```text
AIDoc Revision R103
        ↓
Git Commit abc123
```

---

# 15. CLI 必须做

我甚至建议第一阶段先做 CLI，而不是先做漂亮 UI。

例如：

```bash
aidoc init
aidoc open project.aidoc

aidoc node list
aidoc node show database

aidoc node update database

aidoc history
aidoc diff R103 R105

aidoc revert R103

aidoc validate

aidoc export html
aidoc export markdown
```

这样 Core 很容易测试。

---

# 16. 项目目录

我推荐：

```text
aidoc/
│
├── apps/
│   ├── desktop/
│   │   ├── src/
│   │   └── src-tauri/
│   │
│   └── cli/
│
├── crates/
│   ├── aidoc-core/
│   ├── aidoc-model/
│   ├── aidoc-storage/
│   ├── aidoc-package/
│   ├── aidoc-history/
│   ├── aidoc-operation/
│   ├── aidoc-renderer/
│   ├── aidoc-validator/
│   └── aidoc-exporter/
│
├── packages/
│   ├── editor/
│   ├── ui/
│   └── aidoc-types/
│
├── services/
│   ├── ai/
│   └── rag/
│
├── examples/
│   └── order-system.aidoc
│
└── specs/
    └── v0.1/
```

---

# 17. 最推荐的技术边界

最终可以非常清晰：

```text
                    ┌──────────────┐
                    │ React/Tiptap │
                    │    Editor    │
                    └──────┬───────┘
                           │
                           ↓
                    ┌──────────────┐
                    │ AIDoc API    │
                    └──────┬───────┘
                           │
                    ┌──────▼───────┐
                    │ Rust Core    │
                    │              │
                    │ Model        │
                    │ Operation    │
                    │ Revision     │
                    │ Relation     │
                    │ Validation   │
                    │ Renderer     │
                    └──────┬───────┘
                           │
                     ┌─────▼─────┐
                     │  SQLite   │
                     └─────┬─────┘
                           │
                    ┌──────▼───────┐
                    │ AIDoc Package │
                    │     ZIP       │
                    └───────────────┘


             ┌──────────────────┐
             │ AI / Agent       │
             │ Python / LLM     │
             └────────┬─────────┘
                      │
                 Operation
                      │
                      ↓
                 AIDoc Core
```

## 18. 第一阶段不要做太多

我建议 **AIDoc v0.1 MVP** 只做：

```text
① Rust AIDoc Core
② SQLite
③ ZIP Package
④ manifest.json
⑤ HTML
⑥ Node / Relation
⑦ Operation
⑧ Revision
⑨ Revert
⑩ Tauri + React
⑪ Tiptap
⑫ Mermaid
⑬ CLI
```

暂时不要做：

```text
❌ 多人实时协作
❌ 云同步
❌ Qdrant
❌ MCP
❌ 多 Agent
❌ 复杂权限
❌ 在线服务
❌ 自研 Diagram Editor
```

**最关键的开发顺序是：**

```text
AIDoc Spec
   ↓
Rust Model
   ↓
SQLite Storage
   ↓
Operation Engine
   ↓
Revision / Revert
   ↓
Validator
   ↓
ZIP Package
   ↓
HTML Renderer
   ↓
CLI
   ↓
Tauri + React Editor
   ↓
AI Agent
   ↓
RAG
```

这样做的好处是：**即使 Tauri 编辑器以后推倒重做，AIDoc 格式本身也不会受影响。**

而且这套架构和你之前做的 **Tauri + React + Rust + Python** 路线非常契合：Tauri 负责产品壳，React 负责编辑器，Rust 真正掌握 AIDoc 生命周期，Python/LLM 作为上层智能能力。
