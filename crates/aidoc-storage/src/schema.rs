//! SQL schema + DDL bootstrap.

/// All DDL applied at store open time. Idempotent — uses `IF NOT EXISTS`.
///
/// Mirrors spec §32 tables:
///   documents, nodes, relations, revisions, changes, operations, provenance
/// Plus `snapshots` and `assets` for forward-compat (spec §33, §3.1).
pub const DDL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS documents (
    id         TEXT PRIMARY KEY,
    title      TEXT NOT NULL,
    root_node  TEXT NOT NULL,
    version    TEXT NOT NULL DEFAULT '0.1',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS nodes (
    doc_id        TEXT NOT NULL,
    id            TEXT NOT NULL,
    kind          TEXT NOT NULL,
    parent        TEXT,
    position      INTEGER NOT NULL DEFAULT 0,
    semantic_type TEXT,
    content       TEXT NOT NULL DEFAULT '',
    attributes    TEXT NOT NULL DEFAULT '{}',
    content_hash  TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (doc_id, id),
    FOREIGN KEY (doc_id) REFERENCES documents(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_nodes_parent  ON nodes(doc_id, parent);
CREATE INDEX IF NOT EXISTS idx_nodes_doc     ON nodes(doc_id);

CREATE TABLE IF NOT EXISTS relations (
    doc_id      TEXT NOT NULL,
    id          TEXT NOT NULL,
    source      TEXT NOT NULL,
    target      TEXT NOT NULL,
    kind        TEXT NOT NULL,
    custom_kind TEXT,
    PRIMARY KEY (doc_id, id),
    FOREIGN KEY (doc_id) REFERENCES documents(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_relations_source ON relations(doc_id, source);
CREATE INDEX IF NOT EXISTS idx_relations_target ON relations(doc_id, target);

CREATE TABLE IF NOT EXISTS revisions (
    doc_id     TEXT NOT NULL,
    id         TEXT NOT NULL,
    parent     TEXT,
    operation  TEXT NOT NULL,
    message    TEXT,
    created_at TEXT NOT NULL,
    is_head    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (doc_id, id),
    FOREIGN KEY (doc_id) REFERENCES documents(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_revisions_head ON revisions(doc_id, is_head);

CREATE TABLE IF NOT EXISTS changes (
    doc_id      TEXT NOT NULL,
    id          TEXT NOT NULL,
    revision    TEXT NOT NULL,
    node        TEXT NOT NULL,
    change_type TEXT NOT NULL,
    before_hash TEXT,
    after_hash  TEXT,
    summary     TEXT,
    PRIMARY KEY (doc_id, id)
);
CREATE INDEX IF NOT EXISTS idx_changes_revision ON changes(doc_id, revision);

CREATE TABLE IF NOT EXISTS operations (
    doc_id            TEXT NOT NULL,
    id                TEXT NOT NULL,
    op_type           TEXT NOT NULL,
    target            TEXT,
    expected_revision TEXT NOT NULL,
    target_revision   TEXT,
    actor_json        TEXT NOT NULL,
    patch_json        TEXT,
    reason            TEXT,
    created_at        TEXT NOT NULL,
    PRIMARY KEY (doc_id, id),
    FOREIGN KEY (doc_id) REFERENCES documents(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS provenance (
    doc_id     TEXT NOT NULL,
    op_id      TEXT NOT NULL,
    kind       TEXT NOT NULL,
    detail     TEXT NOT NULL,
    PRIMARY KEY (doc_id, op_id, kind),
    FOREIGN KEY (doc_id, op_id) REFERENCES operations(doc_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS snapshots (
    doc_id     TEXT NOT NULL,
    revision   TEXT NOT NULL,
    created_at TEXT NOT NULL,
    node_count INTEGER NOT NULL,
    payload    TEXT NOT NULL,
    PRIMARY KEY (doc_id, revision)
);

CREATE TABLE IF NOT EXISTS assets (
    doc_id  TEXT NOT NULL,
    path    TEXT NOT NULL,
    kind    TEXT NOT NULL,
    bytes   INTEGER NOT NULL,
    PRIMARY KEY (doc_id, path)
);
"#;

pub const SCHEMA_VERSION: &str = "0.1.0";
