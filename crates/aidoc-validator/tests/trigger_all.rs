//! Validator E2E: deliberately construct a broken fixture and confirm every
//! validator category fires at least once. Run with:
//!   cargo test -p aidoc-validator --test trigger_all

use aidoc_model::{
    Document, Node, NodeId, NodeKind, OpId, Relation, RelationKind, Revision, RevisionId,
};
use aidoc_storage::{AnyhowErr, Store, crud};
use anyhow::Result;

fn doc_id() -> &'static str {
    "validator-test"
}

fn setup() -> Store {
    let mut store = Store::open_memory().expect("open memory");
    let doc = Document::new(doc_id(), "Validator Test", NodeId::from_validated("root"));
    let rev = Revision {
        id: RevisionId::new("R000"),
        parent: None,
        operation: OpId::new("OP-000"),
        created_at: chrono::Utc::now(),
        message: Some("seed".into()),
        branch: None,
    };
    store
        .tx::<_, _, AnyhowErr>(|tx| {
            crud::upsert_document(tx, &doc)?;
            let mut root = Node::new(NodeId::from_validated("root"), NodeKind::Section);
            root.content = "Validator Test".into();
            crud::insert_node(tx, doc_id(), &root)?;
            crud::insert_revision(tx, doc_id(), &rev, true)?;
            Ok::<_, AnyhowErr>(())
        })
        .expect("seed");
    store
}

fn raw_insert(store: &mut Store, id: &str, parent: Option<&str>) -> Result<()> {
    store.tx::<_, _, AnyhowErr>(|tx| {
        tx.execute(
            "INSERT INTO nodes(doc_id, id, kind, parent, position, content, attributes) \
             VALUES(?1, ?2, 'section', ?3, 0, '', '{}')",
            rusqlite::params![doc_id(), id, parent],
        )
        .map_err(anyhow::Error::from)?;
        Ok::<_, AnyhowErr>(())
    })?;
    Ok(())
}

#[test]
fn identity_validator_rejects_empty_node_id() {
    // Bypass the operation engine: insert a row whose id is "" (which
    // NodeId::new rejects, but the raw SQLite column doesn't care).
    let mut store = setup();
    raw_insert(&mut store, "", None).expect("seed bad node");

    let report = aidoc_validator::validate(&store, doc_id()).expect("validate runs");
    assert!(
        !report.identity_errors().is_empty(),
        "identity validator should fire on empty node id, got {:?}",
        report.identity_errors()
    );
}

#[test]
fn structure_validator_rejects_circular_parent() {
    // a -> b -> a (cycle). Direct SQL bypass.
    let mut store = setup();
    raw_insert(&mut store, "a", Some("b")).expect("seed a");
    raw_insert(&mut store, "b", Some("a")).expect("seed b");

    let report = aidoc_validator::validate(&store, doc_id()).expect("validate runs");
    assert!(
        !report.structure_errors().is_empty(),
        "structure validator should fire on circular hierarchy, got {:?}",
        report.structure_errors()
    );
}

#[test]
fn relation_validator_rejects_unknown_target() {
    let mut store = setup();
    let rel = Relation {
        id: "rel-1".into(),
        source: NodeId::from_validated("root"),
        target: NodeId::from_validated("ghost"),
        kind: RelationKind::References,
        custom_kind: None,
    };
    store
        .tx::<_, _, AnyhowErr>(|tx| {
            crud::insert_relation(tx, doc_id(), &rel).expect("insert");
            Ok::<_, AnyhowErr>(())
        })
        .expect("seed relation");

    let report = aidoc_validator::validate(&store, doc_id()).expect("validate runs");
    assert!(
        !report.relation_errors().is_empty(),
        "relation validator should fire when target is missing, got {:?}",
        report.relation_errors()
    );
}

#[test]
fn revision_validator_rejects_missing_parent() {
    let mut store = setup();
    let rev = Revision {
        id: RevisionId::new("R099"),
        parent: Some(RevisionId::new("R042")), // doesn't exist
        operation: OpId::new("OP-099"),
        created_at: chrono::Utc::now(),
        message: Some("orphan".into()),
        branch: None,
    };
    store
        .tx::<_, _, AnyhowErr>(|tx| {
            crud::insert_revision(tx, doc_id(), &rev, false).expect("insert");
            Ok::<_, AnyhowErr>(())
        })
        .expect("seed revision");

    let report = aidoc_validator::validate(&store, doc_id()).expect("validate runs");
    assert!(
        !report.revision_errors().is_empty(),
        "revision validator should fire on missing parent, got {:?}",
        report.revision_errors()
    );
}

#[test]
fn code_ref_validator_rejects_empty_file_attribute() {
    let mut store = setup();
    let mut node = Node::new(NodeId::from_validated("code-ref"), NodeKind::CodeRef);
    node.attributes.insert("file".into(), "".into());
    node.content = "see code".into();
    store
        .tx::<_, _, AnyhowErr>(|tx| {
            crud::insert_node(tx, doc_id(), &node).expect("insert");
            Ok::<_, AnyhowErr>(())
        })
        .expect("seed code-ref");

    let report = aidoc_validator::validate(&store, doc_id()).expect("validate runs");
    assert!(
        !report.code_ref_errors().is_empty(),
        "code_ref validator should fire on empty file attribute, got {:?}",
        report.code_ref_errors()
    );
}
