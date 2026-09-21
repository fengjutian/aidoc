# AIDoc v0.2 Format Specification

**Status:** Draft  
**File extension:** `.aidoc`  
**Media type:** `application/vnd.aidoc+zip`

## 1. Design contract

AIDoc is an open, AI-oriented structured-document container. The portable
contract is JSON plus JSON Schema; SQLite and HTML are implementations and
representations, not the interchange format.

1. Every node has a stable ID.
2. Content changes are typed, local operations.
3. Every accepted operation creates an immutable revision.
4. Revert creates a new revision.
5. Unknown properties must not make the base document unreadable.

## 2. Package layout

```text
example.aidoc
├── manifest.json
├── document/
│   ├── document.json       canonical portable document
│   └── document.html       derived representation
├── schemas/
│   ├── document.schema.json
│   └── operation.schema.json
├── assets/
└── .internal/document.db  optional implementation store
```

`manifest.entry` identifies the canonical entry and is
`document/document.json` for v0.2. Consumers must not assume SQLite is present.

## 3. Canonical document

The canonical entry contains `format`, `format_version`, document metadata,
the current revision, nodes, and relations. Its `$schema` points to the schema
embedded in the same package.

Node `content` is plain text or Markdown-like source appropriate to its kind.
HTML is never required as canonical node content. Renderers may produce HTML.
Attributes are string-valued portable metadata.

Extensions use the base `generic` kind and a namespaced semantic type:

```json
{
  "id": "chart-1",
  "kind": "generic",
  "semantic_type": "com.example.chart",
  "position": 0,
  "content": "{\"series\":[1,2,3]}",
  "attributes": { "media_type": "application/json" }
}
```

This is lossless for consumers that do not render the extension: they retain
its ID, semantic type, source content, and attributes.

## 4. AI operations

AI systems should emit operations conforming to
`schemas/operation.schema.json`. A typical update is:

```json
{
  "$schema": "schemas/operation.schema.json",
  "id": "OP-ai-001",
  "type": "update",
  "target": "introduction",
  "expected_revision": "R012",
  "actor": {
    "type": "operation",
    "actor": { "type": "ai", "agent": "writer", "model": "example/model" }
  },
  "patch": { "content": "Updated introduction." },
  "reason": "Improve clarity"
}
```

The operation engine validates identity, revision, structure, relation and
content-hash constraints before committing. Whole-document rewrites are not
the default mutation mechanism.

## 5. Compatibility

Implementations must continue opening v0.1 packages. When a v0.1 package is
saved by a v0.2 implementation, the canonical JSON entry and embedded schemas
are added, while existing HTML, assets and history remain intact.
