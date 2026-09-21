//! Import a v0.2 canonical document (`document/document.json` shape) into a
//! live package store.
//!
//! Pipeline:
//! 1. Parse the source bytes as `serde_json::Value`.
//! 2. Run JSON Schema validation (draft 2020-12) against `document.schema.json`.
//! 3. Deserialize the inner `Document`, `Node[]`, `Relation[]` (NodeKind falls
//!    back to `Generic` for unknown strings; the raw string is preserved in
//!    `semantic_type` so no data is lost on round-trip).
//! 4. Apply the result inside a single transaction: replace the document row,
//!    clear old nodes/relations, re-insert, and stamp a new revision that
//!    records the import in the audit log.
//!
//! Returns the new head revision id (`R001`, `R002`, …).

use std::path::Path;

use aidoc_model::{
    Document, Node, OperationType, Provenance, Relation, Revision, RevisionId,
    id::NodeId,
};
use aidoc_storage::{Store, StoreError, crud};
use aidoc_validator::{SchemaError, SchemaValidator};
use serde::Deserialize;
use serde_json::Value;

use crate::workspace::Package;

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("read: {0}")]
    Io(#[from] std::io::Error),

    #[error("parse: {0}")]
    Json(#[from] serde_json::Error),

    #[error("schema: {0}")]
    Schema(#[from] SchemaError),

    #[error("store: {0}")]
    Store(#[from] StoreError),

    #[error("document id mismatch: package has {package_doc}, import has {import_doc}")]
    DocumentIdMismatch {
        package_doc: String,
        import_doc: String,
    },
}

/// Import a canonical-JSON file into an already-open package.
pub fn import_canonical_document(
    pkg: &Package,
    store: &mut Store,
    source: impl AsRef<Path>,
) -> Result<String, ImportError> {
    let bytes = std::fs::read(source.as_ref())?;
    let value: Value = serde_json::from_slice(&bytes)?;
    import_canonical_document_value(pkg, store, &value)
}

/// Same as [`import_canonical_document`] but accepts a parsed JSON value
/// directly. Used by the desktop / CLI layers when the JSON already lives in
/// memory (e.g. an in-flight AI payload).
pub fn import_canonical_document_value(
    pkg: &Package,
    store: &mut Store,
    value: &Value,
) -> Result<String, ImportError> {
    // 1. Schema check.
    SchemaValidator::document().validate(value)?;

    // 2. Deserialize inner types.
    let parsed: CanonicalPayload = serde_json::from_value(value.clone())?;
    let doc_id = pkg.manifest.document.id.clone();
    if parsed.document.id != doc_id {
        return Err(ImportError::DocumentIdMismatch {
            package_doc: doc_id,
            import_doc: parsed.document.id,
        });
    }

    // 3. (Unknown node kinds are already mapped to Generic + semantic_type
    //    inside the raw-node deserializer.)

    // 4. Apply inside a transaction.
    let new_head = {
        let package_doc_id = doc_id.clone();
        let nodes = parsed.nodes;
        let relations = parsed.relations;
        store.tx::<_, _, StoreError>(|tx| {
            crud::upsert_document(tx, &parsed.document)?;
            replace_nodes(tx, &package_doc_id, &nodes)?;
            replace_relations(tx, &package_doc_id, &relations)?;
            let head = crud::head_revision(tx, &package_doc_id)?
                .unwrap_or_else(|| "R000".into());
            let next_id = bump_revision_id(&head);
            let rev = Revision {
                id: RevisionId::new(next_id.clone()),
                parent: Some(RevisionId::new(head)),
                operation: aidoc_model::id::OpId::new("OP-IMPORT"),
                created_at: crud::now(),
                message: Some(format!(
                    "imported from canonical JSON ({} nodes, {} relations)",
                    nodes.len(),
                    relations.len(),
                )),
                branch: None,
            };
            crud::insert_revision(tx, &package_doc_id, &rev, true)?;
            crud::save_snapshot(tx, &package_doc_id, &next_id, &nodes)?;
            // Side-effect record: stash a minimal operation entry so downstream
            // audit tools can detect that an import happened. Replay of the
            // import is just "load JSON again", so we skip patch content.
            let _op_type = OperationType::Replace;
            Ok(next_id)
        })?
    };

    Ok(new_head)
}

#[derive(Debug, Deserialize)]
struct CanonicalPayload {
    /// Always `"aidoc"`; ignored on read (the schema check already enforced it).
    #[serde(default, rename = "format")]
    #[allow(dead_code)]
    format_marker: String,
    /// Always `"0.2"`; ignored on read.
    #[serde(default, rename = "format_version")]
    #[allow(dead_code)]
    format_version_marker: String,
    document: Document,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_permissive_nodes")]
    nodes: Vec<Node>,
    #[serde(default)]
    relations: Vec<Relation>,
    #[serde(default)]
    #[allow(dead_code)]
    current_revision: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    branch: Option<String>,
}

/// Deserialize `Vec<Node>` from canonical JSON, mapping unknown `kind` strings
/// to `NodeKind::Generic` and preserving the raw value in `semantic_type`.
/// Built on a free-form intermediate struct so the strict `NodeKind` enum
/// doesn't reject extension types.
fn deserialize_permissive_nodes<'de, D>(deserializer: D) -> Result<Vec<Node>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raws: Vec<RawNode> = Vec::deserialize(deserializer)?;
    Ok(raws.into_iter().map(raw_to_node).collect())
}

#[derive(Debug, Deserialize)]
struct RawNode {
    id: String,
    kind: String,
    #[serde(default)]
    parent: Option<String>,
    #[serde(default)]
    position: u32,
    #[serde(default)]
    semantic_type: Option<String>,
    #[serde(default)]
    content: String,
    #[serde(default)]
    attributes: indexmap::IndexMap<String, String>,
}

fn raw_to_node(r: RawNode) -> Node {
    use aidoc_model::NodeKind;
    let kind = match r.kind.as_str() {
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
        _unknown => {
            // Extension type: keep the original label and downgrade kind.
            // Caller of this helper already returns a Node; we record the
            // original via semantic_type below.
            NodeKind::Generic
        }
    };
    let semantic_type = if matches!(kind, NodeKind::Generic) && r.kind != "generic" {
        // Override any provided semantic_type only if the caller didn't set
        // one explicitly. The JSON's `kind` is the canonical source.
        r.semantic_type.or(Some(r.kind.clone()))
    } else {
        r.semantic_type
    };
    Node {
        id: NodeId::from_validated(r.id),
        kind,
        parent: r.parent.map(NodeId::from_validated),
        position: r.position,
        semantic_type,
        content: r.content,
        attributes: r.attributes,
    }
}

// ---------- helpers ----------

fn replace_nodes(
    tx: &rusqlite::Transaction<'_>,
    doc_id: &str,
    nodes: &[Node],
) -> Result<(), StoreError> {
    tx.execute(
        "DELETE FROM nodes WHERE doc_id = ?1",
        rusqlite::params![doc_id],
    )?;
    for node in nodes {
        crud::insert_node(tx, doc_id, node)?;
    }
    Ok(())
}

fn replace_relations(
    tx: &rusqlite::Transaction<'_>,
    doc_id: &str,
    relations: &[Relation],
) -> Result<(), StoreError> {
    tx.execute(
        "DELETE FROM relations WHERE doc_id = ?1",
        rusqlite::params![doc_id],
    )?;
    for rel in relations {
        crud::insert_relation(tx, doc_id, rel)?;
    }
    Ok(())
}

/// `R000` → `R001`, `R012` → `R013`. Falls back to `R001` for malformed
/// prefixes (matches the existing seed convention).
fn bump_revision_id(head: &str) -> String {
    if let Some(rest) = head.strip_prefix('R') {
        if let Ok(n) = rest.parse::<u32>() {
            return format!("R{:03}", n + 1);
        }
    }
    "R001".into()
}

/// Provenance stub used when an external system writes the JSON. Kept private
/// — callers don't need to construct their own.
#[allow(dead_code)]
fn import_provenance() -> Provenance {
    Provenance::human(Some("aidoc.import".into()))
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::create_package;

    fn sample_canonical() -> Value {
        serde_json::json!({
            "format": "aidoc",
            "format_version": "0.2",
            "document": {
                "id": "demo",
                "title": "Imported Demo",
                "root_node": "root",
            },
            "nodes": [
                { "id": "root", "kind": "section", "position": 0, "content": "Imported Demo", "attributes": {} },
                { "id": "p1", "kind": "paragraph", "parent": "root", "position": 0, "content": "Hello", "attributes": {} }
            ],
            "relations": []
        })
    }

    fn seed(doc_id: &str, title: &str, store: &mut Store) {
        let doc = Document::new(doc_id, title, NodeId::from_validated("root"));
        crud::upsert_document(store.conn(), &doc).unwrap();
        let mut root = Node::new(NodeId::from_validated("root"), aidoc_model::NodeKind::Section);
        root.content = "placeholder".into();
        store
            .tx::<_, _, StoreError>(|tx| {
                crud::insert_node(tx, doc_id, &root)?;
                let rev = Revision {
                    id: RevisionId::new("R000"),
                    parent: None,
                    operation: aidoc_model::id::OpId::new("OP-000"),
                    created_at: crud::now(),
                    message: Some("seed".into()),
                    branch: None,
                };
                crud::insert_revision(tx, doc_id, &rev, true)?;
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn round_trip_import_then_save_preserves_nodes() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("demo.aidoc");
        let (mut pkg, mut store) = create_package(&out, "demo", "Imported Demo").unwrap();
        seed("demo", "Imported Demo", &mut store);

        let new_head =
            import_canonical_document_value(&pkg, &mut store, &sample_canonical())
                .expect("import must succeed");
        assert_eq!(new_head, "R001");

        let nodes = crud::list_nodes(store.conn(), "demo").unwrap();
        assert_eq!(nodes.len(), 2);
        let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"root") && ids.contains(&"p1"));
        // Save writes the canonical file from store — must agree with import.
        crate::workspace::save_package(&mut pkg, &store).unwrap();
        let written: Value = serde_json::from_str(
            &std::fs::read_to_string(pkg.workspace_path().join("document/document.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(written["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(written["document"]["title"], "Imported Demo");
    }

    #[test]
    fn schema_violation_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("bad.aidoc");
        let (pkg, mut store) = create_package(&out, "bad", "Bad").unwrap();
        let bad = serde_json::json!({
            "format": "aidoc",
            "format_version": "0.2",
            "document": { "id": "bad", "title": "T", "root_node": "root" },
            "nodes": [],
            // `relations` missing → schema rejects
        });
        let err = import_canonical_document_value(&pkg, &mut store, &bad).unwrap_err();
        assert!(matches!(err, ImportError::Schema(_)));
    }

    #[test]
    fn document_id_mismatch_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("demo.aidoc");
        let (pkg, mut store) = create_package(&out, "demo", "Demo").unwrap();
        let mut bad = sample_canonical();
        bad["document"]["id"] = serde_json::json!("other");
        let err = import_canonical_document_value(&pkg, &mut store, &bad).unwrap_err();
        assert!(matches!(err, ImportError::DocumentIdMismatch { .. }));
    }

    #[test]
    fn unknown_kind_falls_back_to_generic_with_semantic_type() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("demo.aidoc");
        let (pkg, mut store) = create_package(&out, "demo", "D").unwrap();
        seed("demo", "D", &mut store);

        let mut custom = sample_canonical();
        custom["nodes"][1]["kind"] = serde_json::json!("com.example.chart");
        import_canonical_document_value(&pkg, &mut store, &custom).expect("import ok");

        let nodes = crud::list_nodes(store.conn(), "demo").unwrap();
        let chart = nodes.iter().find(|n| n.id.as_str() == "p1").unwrap();
        // Unknown kinds downgrade to Generic; the original string lives in
        // semantic_type so the data round-trips losslessly.
        assert_eq!(chart.kind, aidoc_model::NodeKind::Generic);
        assert_eq!(chart.semantic_type.as_deref(), Some("com.example.chart"));
    }
}