//! Typed CRUD: row ↔ model conversion. Pure functions, no business logic.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Transaction, params_from_iter, types::Value};

use aidoc_model::{
    Change, ChangeType, Document, HashRef, Node, NodeKind, Relation, RelationKind, Revision,
    id::{NodeId, sha256_hex},
};

use crate::store::StoreError;

/// Current UTC timestamp for write paths.
pub fn now() -> DateTime<Utc> {
    Utc::now()
}

fn kind_to_str(k: NodeKind) -> &'static str {
    match k {
        NodeKind::Section => "section",
        NodeKind::Paragraph => "paragraph",
        NodeKind::Heading => "heading",
        NodeKind::List => "list",
        NodeKind::ListItem => "list-item",
        NodeKind::Table => "table",
        NodeKind::TableRow => "table-row",
        NodeKind::TableCell => "table-cell",
        NodeKind::Code => "code",
        NodeKind::Blockquote => "blockquote",
        NodeKind::Link => "link",
        NodeKind::Image => "image",
        NodeKind::Diagram => "diagram",
        NodeKind::CodeRef => "code-ref",
        NodeKind::Requirement => "requirement",
        NodeKind::Decision => "decision",
        NodeKind::Problem => "problem",
        NodeKind::Solution => "solution",
        NodeKind::Reference => "reference",
        NodeKind::Details => "details",
        NodeKind::Summary => "summary",
        NodeKind::Generic => "generic",
    }
}

fn kind_from_str(s: &str) -> Result<NodeKind, StoreError> {
    Ok(match s {
        "section" => NodeKind::Section,
        "paragraph" => NodeKind::Paragraph,
        "heading" => NodeKind::Heading,
        "list" => NodeKind::List,
        "list-item" => NodeKind::ListItem,
        "table" => NodeKind::Table,
        "table-row" => NodeKind::TableRow,
        "table-cell" => NodeKind::TableCell,
        "code" => NodeKind::Code,
        "blockquote" => NodeKind::Blockquote,
        "link" => NodeKind::Link,
        "image" => NodeKind::Image,
        "diagram" => NodeKind::Diagram,
        "code-ref" => NodeKind::CodeRef,
        "requirement" => NodeKind::Requirement,
        "decision" => NodeKind::Decision,
        "problem" => NodeKind::Problem,
        "solution" => NodeKind::Solution,
        "reference" => NodeKind::Reference,
        "details" => NodeKind::Details,
        "summary" => NodeKind::Summary,
        "generic" => NodeKind::Generic,
        other => return Err(StoreError::Integrity(format!("unknown node kind: {other}"))),
    })
}

fn relation_kind_to_str(k: RelationKind) -> &'static str {
    k.as_str()
}

// ---------- Documents ----------

pub fn upsert_document(conn: &Connection, doc: &Document) -> Result<(), StoreError> {
    let now = now().to_rfc3339();
    conn.execute(
        r#"INSERT INTO documents(id, title, root_node, version, created_at, updated_at)
           VALUES(?1, ?2, ?3, ?4, ?5, ?5)
           ON CONFLICT(id) DO UPDATE SET
               title      = excluded.title,
               root_node  = excluded.root_node,
               version    = excluded.version,
               updated_at = ?5"#,
        rusqlite::params![doc.id, doc.title, doc.root_node.as_str(), doc.version, now],
    )?;
    Ok(())
}

pub fn get_document(conn: &Connection, id: &str) -> Result<Option<Document>, StoreError> {
    let mut stmt = conn.prepare("SELECT title, root_node, version FROM documents WHERE id = ?1")?;
    let mut rows = stmt.query([id])?;
    if let Some(r) = rows.next()? {
        Ok(Some(Document {
            id: id.to_owned(),
            title: r.get::<_, String>(0)?,
            root_node: NodeId::from_validated(r.get::<_, String>(1)?),
            version: r.get::<_, String>(2)?,
        }))
    } else {
        Ok(None)
    }
}

// ---------- Nodes ----------

pub fn insert_node(tx: &Transaction<'_>, doc_id: &str, node: &Node) -> Result<(), StoreError> {
    let attrs = serde_json::to_string(&node.attributes)?;
    let hash = sha256_hex(node.content.as_bytes());
    tx.execute(
        r#"INSERT INTO nodes(doc_id, id, kind, parent, position, semantic_type, content, attributes, content_hash)
           VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
        rusqlite::params![
            doc_id,
            node.id.as_str(),
            kind_to_str(node.kind),
            node.parent.as_ref().map(|p| p.as_str().to_owned()),
            node.position,
            node.semantic_type,
            node.content,
            attrs,
            hash
        ],
    )?;
    Ok(())
}

pub fn update_node(tx: &Transaction<'_>, doc_id: &str, node: &Node) -> Result<String, StoreError> {
    let attrs = serde_json::to_string(&node.attributes)?;
    let hash = sha256_hex(node.content.as_bytes());
    tx.execute(
        r#"UPDATE nodes
           SET kind = ?3,
               parent = ?4,
               position = ?5,
               semantic_type = ?6,
               content = ?7,
               attributes = ?8,
               content_hash = ?9
           WHERE doc_id = ?1 AND id = ?2"#,
        rusqlite::params![
            doc_id,
            node.id.as_str(),
            kind_to_str(node.kind),
            node.parent.as_ref().map(|p| p.as_str().to_owned()),
            node.position,
            node.semantic_type,
            node.content,
            attrs,
            hash
        ],
    )?;
    Ok(hash)
}

pub fn delete_node(tx: &Transaction<'_>, doc_id: &str, node_id: &NodeId) -> Result<(), StoreError> {
    tx.execute(
        "DELETE FROM nodes WHERE doc_id = ?1 AND id = ?2",
        rusqlite::params![doc_id, node_id.as_str()],
    )?;
    Ok(())
}

pub fn get_node(
    conn: &Connection,
    doc_id: &str,
    node_id: &NodeId,
) -> Result<Option<Node>, StoreError> {
    let mut stmt = conn.prepare(
        r#"SELECT id, kind, parent, position, semantic_type, content, attributes
           FROM nodes WHERE doc_id = ?1 AND id = ?2"#,
    )?;
    let mut rows = stmt.query(rusqlite::params![doc_id, node_id.as_str()])?;
    if let Some(r) = rows.next()? {
        let kind: String = r.get::<_, String>(1)?;
        let parent: Option<String> = r.get(2)?;
        let attrs: String = r.get::<_, String>(6)?;
        Ok(Some(Node {
            id: NodeId::from_validated(r.get::<_, String>(0)?),
            kind: kind_from_str(&kind)?,
            parent: parent.map(NodeId::from_validated),
            position: r.get::<_, i64>(3)? as u32,
            semantic_type: r.get(4)?,
            content: r.get(5)?,
            attributes: serde_json::from_str(&attrs)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn list_nodes(conn: &Connection, doc_id: &str) -> Result<Vec<Node>, StoreError> {
    let mut stmt = conn.prepare(
        r#"SELECT id, kind, parent, position, semantic_type, content, attributes
           FROM nodes WHERE doc_id = ?1 ORDER BY parent, position, id"#,
    )?;
    let mut out = Vec::new();
    let mut rows = stmt.query([doc_id])?;
    while let Some(r) = rows.next()? {
        let kind: String = r.get(1)?;
        let parent: Option<String> = r.get(2)?;
        let attrs: String = r.get(6)?;
        out.push(Node {
            id: NodeId::from_validated(r.get::<_, String>(0)?),
            kind: kind_from_str(&kind)?,
            parent: parent.map(NodeId::from_validated),
            position: r.get::<_, i64>(3)? as u32,
            semantic_type: r.get(4)?,
            content: r.get(5)?,
            attributes: serde_json::from_str(&attrs)?,
        });
    }
    Ok(out)
}

pub fn get_content_hash(
    conn: &Connection,
    doc_id: &str,
    node_id: &NodeId,
) -> Result<Option<String>, StoreError> {
    let mut stmt = conn.prepare("SELECT content_hash FROM nodes WHERE doc_id = ?1 AND id = ?2")?;
    let mut rows = stmt.query(rusqlite::params![doc_id, node_id.as_str()])?;
    if let Some(r) = rows.next()? {
        Ok(Some(r.get(0)?))
    } else {
        Ok(None)
    }
}

// ---------- Relations ----------

pub fn insert_relation(
    tx: &Transaction<'_>,
    doc_id: &str,
    rel: &Relation,
) -> Result<(), StoreError> {
    tx.execute(
        r#"INSERT INTO relations(doc_id, id, source, target, kind, custom_kind)
           VALUES(?1, ?2, ?3, ?4, ?5, ?6)"#,
        rusqlite::params![
            doc_id,
            rel.id,
            rel.source.as_str(),
            rel.target.as_str(),
            relation_kind_to_str(rel.kind),
            rel.custom_kind
        ],
    )?;
    Ok(())
}

pub fn delete_relation(tx: &Transaction<'_>, doc_id: &str, rel_id: &str) -> Result<(), StoreError> {
    tx.execute(
        "DELETE FROM relations WHERE doc_id = ?1 AND id = ?2",
        rusqlite::params![doc_id, rel_id],
    )?;
    Ok(())
}

pub fn list_relations(conn: &Connection, doc_id: &str) -> Result<Vec<Relation>, StoreError> {
    let mut stmt = conn
        .prepare("SELECT id, source, target, kind, custom_kind FROM relations WHERE doc_id = ?1")?;
    let mut out = Vec::new();
    let mut rows = stmt.query([doc_id])?;
    while let Some(r) = rows.next()? {
        let custom: Option<String> = r.get(4)?;
        let kind_str: String = r.get(3)?;
        let kind = if kind_str == "custom" {
            RelationKind::Custom
        } else {
            RelationKind::parse(&kind_str)
        };
        out.push(Relation {
            id: r.get::<_, String>(0)?,
            source: NodeId::from_validated(r.get::<_, String>(1)?),
            target: NodeId::from_validated(r.get::<_, String>(2)?),
            kind,
            custom_kind: custom,
        });
    }
    Ok(out)
}

// ---------- Revisions ----------

pub fn insert_revision(
    tx: &Transaction<'_>,
    doc_id: &str,
    rev: &Revision,
    is_head: bool,
) -> Result<(), StoreError> {
    tx.execute(
        r#"INSERT INTO revisions(doc_id, id, parent, operation, message, created_at, is_head)
           VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
        rusqlite::params![
            doc_id,
            rev.id.as_str(),
            rev.parent.as_ref().map(|p| p.as_str().to_owned()),
            rev.operation.as_str(),
            rev.message,
            rev.created_at.to_rfc3339(),
            is_head as i64
        ],
    )?;
    // Side-table: named branch (None == main; we don't store a row for main).
    if let Some(branch) = &rev.branch {
        tx.execute(
            r#"INSERT INTO revision_branches(doc_id, revision, branch, created_at)
               VALUES(?1, ?2, ?3, ?4)"#,
            rusqlite::params![doc_id, rev.id.as_str(), branch, rev.created_at.to_rfc3339(),],
        )?;
    }
    // Side-table: every parent (multi-parent merges). Linear revisions still
    // get one row so all reads can ignore `revisions.parent`.
    let parents: Vec<&str> = rev
        .parent
        .as_ref()
        .map(|p| vec![p.as_str()])
        .unwrap_or_default();
    for (seq, p) in parents.iter().enumerate() {
        tx.execute(
            r#"INSERT INTO revision_parents(doc_id, revision, parent, seq)
               VALUES(?1, ?2, ?3, ?4)"#,
            rusqlite::params![doc_id, rev.id.as_str(), p, seq as i64],
        )?;
    }
    Ok(())
}

/// Set the named branch on an existing revision. Used by Branch operations
/// retroactively (a branch is just a labelled revision in v0.1).
pub fn set_branch(
    tx: &Transaction<'_>,
    doc_id: &str,
    rev_id: &str,
    branch: &str,
) -> Result<(), StoreError> {
    tx.execute(
        r#"INSERT INTO revision_branches(doc_id, revision, branch, created_at)
           VALUES(?1, ?2, ?3, ?4)
           ON CONFLICT(doc_id, revision) DO UPDATE SET branch = excluded.branch"#,
        rusqlite::params![doc_id, rev_id, branch, crate::crud::now().to_rfc3339()],
    )?;
    Ok(())
}

pub fn get_branch(
    conn: &Connection,
    doc_id: &str,
    rev_id: &str,
) -> Result<Option<String>, StoreError> {
    let row: Option<String> = conn
        .query_row(
            "SELECT branch FROM revision_branches WHERE doc_id = ?1 AND revision = ?2",
            rusqlite::params![doc_id, rev_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(row)
}

pub fn list_parents(
    conn: &Connection,
    doc_id: &str,
    rev_id: &str,
) -> Result<Vec<String>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT parent FROM revision_parents WHERE doc_id = ?1 AND revision = ?2 ORDER BY seq",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![doc_id, rev_id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn advance_head(tx: &Transaction<'_>, doc_id: &str, new_head: &str) -> Result<(), StoreError> {
    tx.execute(
        "UPDATE revisions SET is_head = 0 WHERE doc_id = ?1",
        rusqlite::params![doc_id],
    )?;
    tx.execute(
        "UPDATE revisions SET is_head = 1 WHERE doc_id = ?1 AND id = ?2",
        rusqlite::params![doc_id, new_head],
    )?;
    Ok(())
}

pub fn head_revision(conn: &Connection, doc_id: &str) -> Result<Option<String>, StoreError> {
    let mut stmt =
        conn.prepare("SELECT id FROM revisions WHERE doc_id = ?1 AND is_head = 1 LIMIT 1")?;
    let mut rows = stmt.query([doc_id])?;
    if let Some(r) = rows.next()? {
        Ok(Some(r.get(0)?))
    } else {
        Ok(None)
    }
}

pub fn max_revision_seq(conn: &Connection, doc_id: &str) -> Result<u64, StoreError> {
    use rusqlite::OptionalExtension;
    let row: Option<String> = conn
        .query_row(
            "SELECT id FROM revisions WHERE doc_id = ?1 ORDER BY CAST(SUBSTR(id, 2) AS INTEGER) DESC LIMIT 1",
            rusqlite::params![doc_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(match row {
        Some(id) => id.trim_start_matches('R').parse::<u64>().unwrap_or(0) + 1,
        None => 0,
    })
}

pub fn get_revision(
    conn: &Connection,
    doc_id: &str,
    rev_id: &str,
) -> Result<Option<Revision>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, parent, operation, message, created_at FROM revisions WHERE doc_id = ?1 AND id = ?2",
    )?;
    let mut rows = stmt.query(rusqlite::params![doc_id, rev_id])?;
    if let Some(r) = rows.next()? {
        let parent: Option<String> = r.get(1)?;
        let created: String = r.get(4)?;
        let branch = get_branch(conn, doc_id, rev_id)?;
        Ok(Some(Revision {
            id: aidoc_model::id::RevisionId::new(r.get::<_, String>(0)?),
            parent: parent.map(aidoc_model::id::RevisionId::new),
            operation: aidoc_model::id::OpId::new(r.get::<_, String>(2)?),
            message: r.get(3)?,
            created_at: chrono::DateTime::parse_from_rfc3339(&created)?.with_timezone(&Utc),
            branch,
        }))
    } else {
        Ok(None)
    }
}

pub fn list_revisions(conn: &Connection, doc_id: &str) -> Result<Vec<Revision>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, parent, operation, message, created_at FROM revisions WHERE doc_id = ?1 ORDER BY created_at ASC",
    )?;
    let mut out = Vec::new();
    let mut rows = stmt.query([doc_id])?;
    while let Some(r) = rows.next()? {
        let parent: Option<String> = r.get(1)?;
        let created: String = r.get(4)?;
        let id = aidoc_model::id::RevisionId::new(r.get::<_, String>(0)?);
        let branch = get_branch(conn, doc_id, id.as_str())?;
        out.push(Revision {
            id,
            parent: parent.map(aidoc_model::id::RevisionId::new),
            operation: aidoc_model::id::OpId::new(r.get::<_, String>(2)?),
            message: r.get(3)?,
            created_at: chrono::DateTime::parse_from_rfc3339(&created)?.with_timezone(&Utc),
            branch,
        });
    }
    Ok(out)
}

// ---------- Changes ----------

pub fn insert_change(tx: &Transaction<'_>, doc_id: &str, ch: &Change) -> Result<(), StoreError> {
    let before = ch.before.as_ref().map(|h| h.hash.clone());
    let after = ch.after.as_ref().map(|h| h.hash.clone());
    tx.execute(
        r#"INSERT INTO changes(doc_id, id, revision, node, change_type, before_hash, after_hash, summary)
           VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        rusqlite::params![
            doc_id,
            ch.id,
            ch.revision.as_str(),
            ch.node.as_str(),
            change_type_str(ch.change_type),
            before,
            after,
            ch.summary
        ],
    )?;
    Ok(())
}

fn change_type_str(t: ChangeType) -> &'static str {
    match t {
        ChangeType::ContentUpdate => "content-update",
        ChangeType::Create => "create",
        ChangeType::Delete => "delete",
        ChangeType::Move => "move",
        ChangeType::Rename => "rename",
        ChangeType::Split => "split",
        ChangeType::Merge => "merge",
        ChangeType::Revert => "revert",
        ChangeType::RelationAdd => "relation-add",
        ChangeType::RelationRemove => "relation-remove",
    }
}

pub fn list_changes_for_revision(
    conn: &Connection,
    doc_id: &str,
    rev_id: &str,
) -> Result<Vec<Change>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, revision, node, change_type, before_hash, after_hash, summary FROM changes WHERE doc_id = ?1 AND revision = ?2",
    )?;
    let mut out = Vec::new();
    let mut rows = stmt.query(rusqlite::params![doc_id, rev_id])?;
    while let Some(r) = rows.next()? {
        let before: Option<String> = r.get(4)?;
        let after: Option<String> = r.get(5)?;
        let ct: String = r.get(3)?;
        out.push(Change {
            id: r.get::<_, String>(0)?,
            revision: aidoc_model::id::RevisionId::new(r.get::<_, String>(1)?),
            node: NodeId::from_validated(r.get::<_, String>(2)?),
            change_type: match ct.as_str() {
                "content-update" => ChangeType::ContentUpdate,
                "create" => ChangeType::Create,
                "delete" => ChangeType::Delete,
                "move" => ChangeType::Move,
                "rename" => ChangeType::Rename,
                "split" => ChangeType::Split,
                "merge" => ChangeType::Merge,
                "revert" => ChangeType::Revert,
                "relation-add" => ChangeType::RelationAdd,
                "relation-remove" => ChangeType::RelationRemove,
                other => {
                    return Err(StoreError::Integrity(format!(
                        "unknown change_type {other}"
                    )));
                }
            },
            before: before.map(|h| HashRef { hash: h }),
            after: after.map(|h| HashRef { hash: h }),
            summary: r.get(6)?,
        });
    }
    Ok(out)
}

// silence unused import warnings when binary builds drop helpers
#[allow(dead_code)]
fn _params_helper(values: Vec<Value>) -> impl rusqlite::Params {
    params_from_iter(values)
}

// ---------- Snapshots ----------

pub fn save_snapshot(
    tx: &Transaction<'_>,
    doc_id: &str,
    rev_id: &str,
    nodes: &[Node],
) -> Result<(), StoreError> {
    let payload = serde_json::to_string(nodes)?;
    tx.execute(
        r#"INSERT INTO snapshots(doc_id, revision, created_at, node_count, payload)
           VALUES(?1, ?2, ?3, ?4, ?5)
           ON CONFLICT(doc_id, revision) DO UPDATE SET
               payload = excluded.payload,
               node_count = excluded.node_count"#,
        rusqlite::params![
            doc_id,
            rev_id,
            crate::crud::now().to_rfc3339(),
            nodes.len() as i64,
            payload
        ],
    )?;
    Ok(())
}

pub fn load_snapshot(
    conn: &Connection,
    doc_id: &str,
    rev_id: &str,
) -> Result<Option<Vec<Node>>, StoreError> {
    let mut stmt =
        conn.prepare("SELECT payload FROM snapshots WHERE doc_id = ?1 AND revision = ?2")?;
    let mut rows = stmt.query(rusqlite::params![doc_id, rev_id])?;
    if let Some(r) = rows.next()? {
        let raw: String = r.get(0)?;
        Ok(Some(serde_json::from_str(&raw)?))
    } else {
        Ok(None)
    }
}
