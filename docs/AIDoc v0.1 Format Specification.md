下面直接给你一版可以作为项目 `SPEC.md` 初稿的 **AIDoc v0.1 Format Specification**。重点不是把格式做复杂，而是先把 **身份、结构、局部修改、关系、历史、AI 操作、可恢复性** 定死。

# AIDoc v0.1 Format Specification

**Status:** Draft
**Version:** 0.1
**File Extension:** `.aidoc`
**MIME Type:** `application/vnd.aidoc`（暂定）

---

## 1. Overview

AIDoc 是一种面向 AI 原生文档的结构化文档格式。

AIDoc 不只是文本格式，而是一个可被人和 AI 持续读取、修改、关联、追踪和恢复的 **Versioned Document Object Model**。

核心模型：

```text
AIDoc
├── Document
├── Nodes
├── Relations
├── Revisions
├── Changes
├── Operations
└── Provenance
```

AIDoc 的设计目标：

* AI 可理解
* 人可阅读
* 支持局部修改
* 支持稳定节点 ID
* 支持超文本链接
* 支持代码引用
* 支持 Mermaid / Diagram
* 支持版本历史
* 支持 AI 操作记录
* 支持精确回退
* 支持冲突检测
* 支持 Code Wiki
* 支持 RAG
* 支持 HTML / Markdown 等格式导出

---

# 2. Design Principles

AIDoc v0.1 遵循以下原则。

### 2.1 HTML First

AIDoc 优先使用标准 HTML 表达文档结构。

例如：

```html
<h1>订单系统</h1>

<section id="architecture">
  <h2>系统架构</h2>

  <p>系统采用微服务架构。</p>
</section>
```

只有 HTML 无法表达的语义，才增加 AIDoc 扩展。

---

### 2.2 Stable Identity

文档节点必须拥有稳定 ID。

```html
<section id="database">
  <h2>数据库设计</h2>
</section>
```

AI 修改：

```text
target = "database"
```

而不是：

```text
line = 132
```

因此文档即使发生大量文字变化，节点仍然可以被准确定位。

---

### 2.3 Local Operation

AIDoc 不允许 AI 默认通过“重写整个 HTML”修改文档。

推荐：

```text
AI
 ↓
Operation
 ↓
Target Node
 ↓
Patch
 ↓
Validation
 ↓
Transaction
 ↓
Revision
```

例如：

```json
{
  "operation": "update",
  "target": "database",
  "expected_revision": "R103",
  "patch": {
    "content": "系统使用 MySQL 8.4。"
  }
}
```

---

### 2.4 History Is Immutable

历史 Revision 不修改、不删除。

回退也是一次新的操作。

```text
R001
 ↓
R002
 ↓
R003
 ↓
R004
 ↓
R005
```

如果 R005 回退 R002：

```text
R005
operation = revert
target_revision = R002
parent = R004
```

而不是删除 R003、R004。

---

### 2.5 Human and AI Use the Same Operation Model

人修改：

```text
Operation
actor.type = human
```

AI 修改：

```text
Operation
actor.type = ai
```

Agent 修改：

```text
Operation
actor.type = agent
```

因此历史系统不区分“谁修改的”，只记录：

> 谁，在什么上下文，以什么操作，改变了什么。

---

# 3. AIDoc Package

`.aidoc` 是一个 ZIP Container。

示例：

```text
example.aidoc
│
├── manifest.json
│
├── document/
│   └── document.html
│
├── .internal/
│   └── document.db
│
├── assets/
│   ├── images/
│   ├── diagrams/
│   └── attachments/
│
└── sources/
```

---

## 3.1 Package Structure

| Path                     | Required | Description            |
| ------------------------ | -------: | ---------------------- |
| `manifest.json`          |      YES | Package metadata       |
| `document/document.html` |      YES | HTML representation    |
| `.internal/document.db`  |     YES* | SQLite canonical store |
| `assets/`                |       NO | Images、附件等             |
| `sources/`               |       NO | 外部来源或缓存资源              |

`*` AIDoc v0.1 Full Profile 要求。

---

# 4. Package Philosophy

AIDoc 的三个层次：

```text
Logical Model
       ↓
SQLite Storage
       ↓
ZIP Package
```

三者不能混为一谈。

### Logical Model

定义：

```text
Document
Node
Relation
Revision
Change
Operation
Provenance
```

### SQLite

是 v0.1 推荐的内部实现。

负责：

* 查询
* 事务
* Revision
* Operation
* Relation
* Node
* Patch
* Provenance

### ZIP

负责：

* 分发
* 复制
* 备份
* 归档
* 文件交换

因此：

> SQLite 不是 AIDoc 标准本身，而是 AIDoc v0.1 的 Canonical Storage Profile。

未来可以出现：

```text
AIDoc + PostgreSQL
AIDoc + IndexedDB
AIDoc + SQLite
AIDoc + Cloud Storage
```

只要实现相同的逻辑模型即可。

---

# 5. Manifest

`manifest.json`：

```json
{
  "format": "aidoc",
  "version": "0.1",

  "document": {
    "id": "doc-order-system",
    "title": "订单系统技术文档"
  },

  "entry": "document/document.html",

  "storage": {
    "type": "sqlite",
    "path": ".internal/document.db"
  },

  "revision": {
    "current": "R105"
  },

  "created_at": "2026-09-18T08:00:00Z",
  "updated_at": "2026-09-18T10:00:00Z"
}
```

---

# 6. Document

Document 是 AIDoc 的根对象。

```json
{
  "id": "doc-order-system",
  "title": "订单系统技术文档",
  "root_node": "root"
}
```

Document ID 在整个生命周期内保持稳定。

---

# 7. Node

Node 是 AIDoc 最核心的对象。

Node 可以表示：

* 标题
* 段落
* Section
* 表格
* 代码
* Diagram
* Requirement
* Decision
* Problem
* Solution
* Reference

基本模型：

```json
{
  "id": "database",
  "type": "section",
  "parent": "architecture",
  "position": 2
}
```

---

# 8. Node Identity

节点 ID 必须：

1. 在 Document 内唯一
2. 稳定
3. 不因为文字修改而变化
4. 不依赖 DOM position
5. 不依赖行号

例如：

```html
<section id="database">
```

AI 可以：

```text
UPDATE database
```

即使原来：

```html
<h2>数据库设计</h2>
```

变成：

```html
<h2>数据库架构</h2>
```

ID 仍然：

```text
database
```

---

# 9. Node Lifecycle

AIDoc v0.1 支持：

```text
CREATE
UPDATE
DELETE
MOVE
RENAME
SPLIT
MERGE
```

例如 Split：

```json
{
  "operation": "split",
  "source": "database",
  "targets": [
    "mysql",
    "redis"
  ]
}
```

原：

```text
database
```

变成：

```text
database
├── mysql
└── redis
```

---

# 10. HTML Representation

AIDoc 使用 HTML 作为主要文档表达层。

优先支持：

```html
html
head
body

h1
h2
h3
h4
h5
h6

p
section
article

ul
ol
li

table
thead
tbody
tr
th
td

pre
code
blockquote

a
img

details
summary
```

---

# 11. AIDoc Semantic Extensions

标准 HTML 无法表达的对象，可以使用 AIDoc Extension。

例如：

```html
<requirement id="REQ-001">
  系统必须支持订单查询。
</requirement>
```

```html
<decision id="DEC-001">
  系统采用 MySQL 作为主数据库。
</decision>
```

```html
<problem id="PROB-001">
  订单查询存在 N+1 查询问题。
</problem>
```

```html
<solution id="SOL-001">
  使用批量查询替代逐条查询。
</solution>
```

扩展对象必须拥有稳定 `id`。

---

# 12. Semantic Type

除了自定义元素，也允许：

```html
<section
  id="database"
  data-aidoc-type="architecture-component">
```

因此：

```text
HTML Structure
+
AIDoc Semantic Metadata
```

共同构成 AIDoc 文档。

---

# 13. Hyperlinks

AIDoc 保留 HTML 原生链接。

内部链接：

```html
<a href="#database">
  数据库设计
</a>
```

外部链接：

```html
<a href="https://example.com">
  官方文档
</a>
```

其他 AIDoc：

```html
<a href="./api.aidoc">
  API 文档
</a>
```

---

# 14. Semantic Relations

除了普通 hyperlink，AIDoc 支持 Relation。

例如：

```html
<ref
  id="ref-db"
  target="database"
  relation="depends-on">
  数据库设计
</ref>
```

关系类型包括：

```text
references
related-to
depends-on
implements
implemented-by
derived-from
supersedes
tested-by
documents
documents-code
```

---

# 15. Document Graph

Relation 可以形成知识图谱：

```text
Requirement
      │
      │ implements
      ↓
     API
      │
      │ implemented-by
      ↓
    Code
      │
      │ tested-by
      ↓
    Test
```

这也是 AIDoc 与普通 Markdown 的重要区别。

---

# 16. Code Reference

Code Wiki 是 AIDoc 的重要使用场景。

示例：

```html
<code-ref
  id="order-service"
  file="src/order/order.service.ts"
  symbol="OrderService">

  <p>
    订单服务负责订单创建和查询。
  </p>

</code-ref>
```

可以进一步记录：

```json
{
  "file": "src/order/order.service.ts",
  "symbol": "OrderService",
  "commit": "9f910ed",
  "lines": {
    "start": 20,
    "end": 180
  }
}
```

其中：

> symbol 优先于 line。

因为代码行号会频繁变化。

---

# 17. Code Synchronization

未来支持：

```text
Git Commit
     ↓
Code Change
     ↓
CodeRef Detection
     ↓
Affected AIDoc Nodes
     ↓
AI Analysis
     ↓
Document Update
```

例如：

```text
src/order/order.service.ts
          ↓
OrderService
          ↓
#order-service
          ↓
architecture/order-service
```

---

# 18. Diagram

Diagram 是一等对象。

v0.1 优先支持 Mermaid。

```html
<diagram
  id="order-flow"
  type="flowchart"
  engine="mermaid">

graph TD
    A[创建订单]
    B[库存检查]
    C[扣库存]
    D[订单创建]

    A --> B
    B --> C
    C --> D

</diagram>
```

Diagram 必须拥有自己的 ID。

---

# 19. Diagram Semantics

Diagram 不应该只是：

```html
<pre>
graph TD
...
</pre>
```

而应该被识别为：

```text
Node
 ├── type = diagram
 ├── engine = mermaid
 ├── diagram_type = flowchart
 └── source
```

这样 AI 可以执行：

```text
UPDATE order-flow
```

而不是修改整个 HTML。

---

# 20. Operation

Operation 描述：

> 谁对什么对象执行了什么动作。

基本结构：

```json
{
  "id": "OP-105",
  "type": "update",
  "actor": {
    "type": "ai",
    "id": "doc-agent"
  },
  "target": "database",
  "expected_revision": "R103",
  "timestamp": "2026-09-18T10:20:00Z"
}
```

---

# 21. Operation Types

v0.1：

```text
CREATE
UPDATE
DELETE
MOVE
RENAME
REPLACE
LINK
UNLINK
SPLIT
MERGE
REVERT
```

---

# 22. Patch

Operation 不直接保存整个 Document。

而是保存 Patch。

例如：

```json
{
  "operation": "update",
  "target": "database",
  "patch": {
    "content": "系统使用 MySQL 8.4。"
  }
}
```

或者：

```json
{
  "operation": "rename",
  "target": "database",
  "patch": {
    "title": "数据库架构"
  }
}
```

---

# 23. Revision

Revision 表示一次完整的文档状态。

```json
{
  "id": "R105",
  "parent": "R104",
  "operation": "OP-105",
  "created_at": "2026-09-18T10:20:00Z"
}
```

关系：

```text
Operation
    ↓
Change
    ↓
Revision
    ↓
Document State
```

---

# 24. Change

Change 描述：

> 文档到底发生了什么变化。

例如：

```json
{
  "id": "CH-105",
  "revision": "R105",
  "node": "database",
  "type": "content-update",

  "before": {
    "hash": "sha256:aaa"
  },

  "after": {
    "hash": "sha256:bbb"
  }
}
```

因此：

```text
Operation = 做了什么
Change    = 改变了什么
Revision  = 改完以后是什么状态
```

三个概念不能混在一起。

---

# 25. Optimistic Concurrency

AI 修改节点时必须可以指定：

```json
{
  "target": "database",
  "expected_revision": "R103"
}
```

如果当前已经是：

```text
R104
```

则 Operation 不应该直接执行。

系统应该返回：

```text
CONFLICT
```

例如：

```json
{
  "status": "conflict",
  "expected_revision": "R103",
  "actual_revision": "R104"
}
```

这样可以避免：

```text
Human 修改
      ↓
AI 使用旧上下文
      ↓
覆盖 Human 修改
```

---

# 26. Content Hash

节点可以保存内容 Hash：

```text
sha256:...
```

Operation 可以同时验证：

```json
{
  "expected_revision": "R103",
  "expected_hash": "sha256:abc..."
}
```

形成：

```text
Revision Check
+
Content Check
```

---

# 27. Revert

AIDoc 不允许通过删除历史实现回退。

错误方式：

```text
删除 R103
删除 R104
恢复 R102
```

正确方式：

```text
R102
 ↓
R103
 ↓
R104
 ↓
R105
```

其中：

```json
{
  "id": "OP-105",
  "type": "revert",
  "target_revision": "R102",
  "parent_revision": "R104"
}
```

因此：

> Revert 本身也是历史。

---

# 28. Provenance

AI 创建内容必须可以追溯来源。

例如：

```json
{
  "type": "code",
  "file": "src/order/order.service.ts",
  "symbol": "OrderService",
  "commit": "9f910ed"
}
```

也可以：

```json
{
  "type": "user-input"
}
```

或者：

```json
{
  "type": "external-document",
  "uri": "..."
}
```

---

# 29. AI Provenance

AI Operation 可以记录：

```json
{
  "actor": {
    "type": "ai",
    "agent": "architecture-agent",
    "model": "..."
  },

  "task": {
    "id": "TASK-2026-001"
  }
}
```

可选记录：

```text
prompt
tool calls
model
temperature
input references
output
reasoning summary
```

其中完整 Prompt 不建议写入 HTML 正文。

应该放在 Operation metadata 中。

---

# 30. AI Operation Record

例如：

```json
{
  "id": "OP-204",

  "type": "update",

  "actor": {
    "type": "ai",
    "agent": "code-wiki-agent",
    "model": "..."
  },

  "target": "order-service",

  "reason": "Source code changed",

  "provenance": [
    "src/order/order.service.ts"
  ],

  "expected_revision": "R203",

  "patch": {
    "content": "订单服务现在支持批量查询。"
  }
}
```

这样未来可以回答：

> 为什么这句话出现在文档里？

系统可以追溯：

```text
Document Node
 ↓
Change
 ↓
Operation
 ↓
AI Agent
 ↓
Code Commit
```

---

# 31. Transaction

一次 Operation 必须是原子的。

推荐：

```sql
BEGIN TRANSACTION;

UPDATE nodes;

INSERT INTO changes;

INSERT INTO operations;

INSERT INTO revisions;

UPDATE manifest;

COMMIT;
```

任何一步失败：

```text
ROLLBACK
```

不能出现：

```text
文档已经修改
但是历史没有记录
```

---

# 32. SQLite Internal Schema

v0.1 推荐：

```text
documents
nodes
relations
revisions
changes
operations
provenance
snapshots
assets
```

关系：

```text
documents
    │
    ├── nodes
    │     └── relations
    │
    ├── revisions
    │      └── changes
    │
    ├── operations
    │
    └── provenance
```

---

# 33. Snapshot

如果每一次 Revision 都保存完整 HTML：

```text
R001 → 1 MB
R002 → 1 MB
R003 → 1 MB
...
R10000 → 1 MB
```

空间会迅速膨胀。

因此推荐：

```text
Snapshot
+
Patch
```

例如：

```text
Snapshot R100
 ↓
Patch R101
 ↓
Patch R102
 ↓
Patch R103
```

定期生成：

```text
R200 Snapshot
R300 Snapshot
R400 Snapshot
```

---

# 34. Branch

未来支持文档分支：

```text
main
 │
 R100
 ├───────────────┐
 │               │
 ↓               ↓
R101            R101-A
 │               │
 ↓               ↓
R102            R102-A
```

用途：

* AI 草稿
* 实验性修改
* 产品方案
* 文档审核

---

# 35. Merge

两个 Branch 可以：

```text
main
  │
  ├── human branch
  │
  └── ai branch
          ↓
        merge
```

Merge 必须产生新的 Revision：

```text
R200
 /  \
R201 R201-A
 \  /
 R202
```

---

# 36. Conflict

Conflict 至少包括：

```text
NODE_CONFLICT
CONTENT_CONFLICT
STRUCTURE_CONFLICT
RELATION_CONFLICT
REVISION_CONFLICT
```

例如：

```text
Human:
database = MySQL 8.0

AI:
database = PostgreSQL
```

不能静默覆盖。

---

# 37. Staleness

AIDoc 特别需要解决 Code Wiki 的“过期”。

例如：

```text
CodeRef
   ↓
Git Commit abc
```

代码已经变成：

```text
Git Commit def
```

则：

```text
CodeRef = stale
```

状态：

```text
synced
stale
conflict
unknown
```

---

# 38. RAG

AIDoc 的 RAG 不应该只切：

```text
500 tokens / chunk
```

而应该保留：

```text
document_id
node_id
parent_id
revision
content_hash
node_type
relations
provenance
```

例如：

```text
Vector
  ↓
node_id
  ↓
parent
  ↓
related nodes
  ↓
code refs
  ↓
source
```

因此可以实现：

> Structure-aware RAG

---

# 39. Security

由于 AIDoc 使用 HTML：

默认：

```text
JavaScript = disabled
```

禁止：

```html
<script>
```

以及：

```text
javascript:
```

事件属性：

```html
onclick=
onload=
onerror=
```

外部 iframe、资源和脚本应该经过 Sandbox / Permission 控制。

原则：

> AIDoc 是文档，不应该因为 AI 写入了一段 HTML 就获得任意代码执行能力。

---

# 40. Validation

AIDoc Validator 至少检查：

### Identity

```text
duplicate node id
invalid id
missing root
```

### Structure

```text
invalid parent
circular hierarchy
orphan node
```

### Relation

```text
missing target
invalid relation
broken reference
```

### Revision

```text
missing parent revision
invalid operation
invalid target revision
```

### CodeRef

```text
missing file
invalid symbol
invalid commit
```

---

# 41. Export

AIDoc 必须支持：

```text
.aidoc
   │
   ├── HTML
   ├── Markdown
   ├── DOCX
   └── PDF
```

其中：

```text
AIDoc
  ↓
Semantic Model
  ↓
Exporter
```

而不是：

```text
AIDoc
 ↓
HTML
 ↓
Markdown
```

这样可以减少格式转换造成的信息损失。

---

# 42. HTML Export

导出静态 HTML：

```text
document.html
assets/
```

浏览器可以直接打开。

AIDoc 特有结构可以降级：

```html
<requirement>
```

转换成：

```html
<section class="requirement">
```

保证基本可读性。

---

# 43. Markdown Export

例如：

```html
<requirement id="REQ-001">
系统必须支持订单查询。
</requirement>
```

可以导出：

```markdown
> **Requirement**

系统必须支持订单查询。
```

但：

> Markdown 导出可能丢失部分 AIDoc 语义，因此不是完整等价格式。

---

# 44. Compatibility

AIDoc 使用：

```text
major.minor
```

例如：

```text
0.1
0.2
1.0
```

原则：

### Minor

允许：

```text
新增字段
新增可选语义
```

旧客户端应该忽略未知字段。

### Major

可能：

```text
修改核心数据模型
修改 Revision semantics
修改 Operation semantics
```

需要升级解析器。

---

# 45. Minimal AIDoc Example

一个最小 AIDoc：

```text
example.aidoc
├── manifest.json
├── document/
│   └── document.html
└── .internal/
    └── document.db
```

`document.html`：

```html
<!DOCTYPE html>
<html>

<head>
  <meta charset="utf-8">
  <title>订单系统</title>
</head>

<body>

  <h1 id="root">订单系统</h1>

  <section id="architecture">

    <h2 id="architecture-title">
      系统架构
    </h2>

    <p id="architecture-description">
      系统采用微服务架构。
    </p>

    <diagram
      id="order-flow"
      type="flowchart"
      engine="mermaid">

      graph TD
        A[创建订单]
        B[库存检查]
        C[订单创建]

        A --> B
        B --> C

    </diagram>

  </section>

  <section id="database">

    <h2>数据库设计</h2>

    <code-ref
      id="order-service"
      file="src/order/order.service.ts"
      symbol="OrderService">

      <p>
        订单服务负责订单创建。
      </p>

    </code-ref>

  </section>

</body>

</html>
```

---

# 46. AI 修改示例

用户：

> 把数据库版本改成 MySQL 8.4。

AI 不应该返回整个 HTML。

而应该产生：

```json
{
  "id": "OP-301",

  "type": "update",

  "actor": {
    "type": "ai",
    "agent": "document-agent"
  },

  "target": "database",

  "expected_revision": "R300",

  "patch": {
    "content": "系统使用 MySQL 8.4。"
  }
}
```

系统：

```text
validate
   ↓
revision check
   ↓
hash check
   ↓
SQLite transaction
   ↓
R301
```

---

# 47. AI + Human Collaboration

AIDoc 最终形成：

```text
                ┌──────────┐
                │   Human  │
                └────┬─────┘
                     │
                     ↓
                Operation
                     ↑
                     │
                ┌────┴─────┐
                │    AI    │
                └──────────┘
                     │
                     ↓
                 Validator
                     │
                     ↓
                 Conflict
                   Check
                     │
                     ↓
                 SQLite
                 Transaction
                     │
                     ↓
                  Revision
                     │
          ┌──────────┴──────────┐
          ↓                     ↓
       Document               History
          │                     │
          ↓                     ↓
        HTML                  Audit
```

---

# 48. Core Data Model

最终可以把 AIDoc v0.1 浓缩成：

```text
Document
│
├── Node
│    ├── Content
│    ├── Metadata
│    └── Identity
│
├── Relation
│
├── Revision
│    └── Change
│
├── Operation
│
└── Provenance
```

其中最重要的链路：

```text
Human / AI
      ↓
 Operation
      ↓
   Change
      ↓
  Revision
      ↓
 Document State
```

以及知识关系：

```text
Node
 ├── parent
 ├── child
 ├── relation
 ├── code-ref
 ├── provenance
 └── revision
```

---

# 49. AIDoc v0.1 Non-Goals

v0.1 **不试图一次解决所有问题**。

暂不定义：

* 实时多人协作协议
* 云端同步协议
* AI Model API
* Agent Protocol
* MCP 协议
* 权限中心
* 分布式版本控制
* 向量数据库格式
* 搜索引擎实现
* 在线评论系统
* 复杂 UI Component 标准

这些可以在后续规范中定义。

---

# 50. Recommended Profiles

为了避免 v0.1 过重，可以定义 Profile。

```text
AIDoc
│
├── Core
│
├── HTML Profile
│
├── Full Package Profile
│
├── Code Wiki Profile
│
├── AI Operation Profile
│
└── RAG Profile
```

例如：

### Core

必须支持：

```text
Document
Node
Revision
Operation
```

### Code Wiki

增加：

```text
CodeRef
Git
Symbol
Staleness
```

### AI Profile

增加：

```text
Agent
Model
Task
Provenance
Operation
```

---

# 51. AIDoc 与 Markdown 的关系

不是：

```text
AIDoc = Markdown 2.0
```

更准确的是：

```text
Markdown
    ↓
Document Text
```

而：

```text
AIDoc
    ↓
Document Object Model
    ├── Content
    ├── Structure
    ├── Relation
    ├── History
    ├── Operation
    └── Provenance
```

Markdown 可以作为：

```text
Import
Export
```

而不是 AIDoc 的核心存储模型。

---

# 52. AIDoc 与 HTML 的关系

也不是：

```text
AIDoc = HTML + 几个标签
```

而是：

```text
HTML
 ↓
Presentation + Basic Structure

AIDoc
 ↓
Semantic Document Model
 ↓
HTML Representation
```

因此 HTML 是 AIDoc 非常重要的**表现与交换层**，但不是完整的历史和操作模型。

---

# 53. AIDoc 的核心价值

最终可以用一个公式概括：

```text
AIDoc
=
Content
+
Structure
+
Identity
+
Relation
+
History
+
Operation
+
Provenance
```

而 `.aidoc`：

```text
.aidoc
=
ZIP
+
HTML
+
SQLite
+
Assets
+
Manifest
```

两者不要混淆：

```text
AIDoc = Logical Format
.aidoc = Physical Package
```

---

# 54. v0.1 最重要的四条规范

如果第一版只真正落实四件事，我建议把下面四条作为 **MUST**：

### MUST 1：节点必须有稳定 ID

```html
<section id="database">
```

### MUST 2：修改必须通过 Operation

```text
AI → Operation → Node → Patch
```

不能默认全文重写。

### MUST 3：每次修改必须产生 Revision

```text
Operation → Change → Revision
```

### MUST 4：Revert 不能删除历史

```text
Revert = New Operation + New Revision
```

这四条实际上决定了 AIDoc 与普通 HTML / Markdown / Wiki 的本质区别。

---

## 55. 最终架构

```text
                         AIDoc
                           │
             ┌─────────────┴─────────────┐
             │                           │
       Logical Model                Package
             │                           │
    ┌────────┼────────┐          ┌───────┼───────┐
    ↓        ↓        ↓          ↓       ↓       ↓
 Document   Node   Relation     HTML   SQLite   Assets
             │
      ┌──────┼──────┐
      ↓      ↓      ↓
   History Operation Provenance
      │
   ┌──┴───┐
   ↓      ↓
Revision Change
   │
   ↓
Document State
```

**这版 v0.1 已经可以作为 AIDoc 项目的第一份正式规范骨架。**


