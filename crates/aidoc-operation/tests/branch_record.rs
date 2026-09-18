//! Branch operation integration test (spec §34).
//!
//! Verifies that a `Branch` op:
//!   1. Is accepted by the apply engine.
//!   2. Produces a new Revision.
//!   3. Tags the new Revision with the branch name in the side-table.
//!   4. Does NOT mutate the live node list (it's just a label).
//!   5. Validates the branch name is non-empty and not "main".

use aidoc_model::{
    Document, Node, NodeId, NodeKind, OpId, Operation, OperationType, Patch, Provenance, Revision,
    RevisionId,
};
use aidoc_operation::apply_operation;
use aidoc_storage::{AnyhowErr, Store, crud};
use indexmap::IndexMap;

fn setup() -> Store {
    let mut store = Store::open_memory().expect("open memory");
    let doc = Document::new("test-doc", "Test Doc", NodeId::from_validated("root"));
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
            root.content = "Test Doc".into();
            crud::insert_node(tx, "test-doc", &root)?;
            crud::insert_revision(tx, "test-doc", &rev, true)?;
            Ok::<_, AnyhowErr>(())
        })
        .expect("seed");
    store
}

fn branch_op(id: &str, expected: &str, name: &str) -> Operation {
    let mut attrs = IndexMap::new();
    attrs.insert("branch".into(), name.into());
    Operation {
        id: OpId::new(id),
        op_type: OperationType::Branch,
        target: None,
        expected_revision: RevisionId::new(expected),
        target_revision: None,
        targets: vec![],
        actor: Provenance::human(Some("alice".into())),
        patch: Some(Patch {
            content: None,
            title: None,
            semantic_type: None,
            attributes: attrs,
        }),
        reason: Some("start ai draft".into()),
    }
}

#[test]
fn branch_records_label_without_mutation() {
    let mut store = setup();
    let before = crud::list_nodes(store.conn(), "test-doc").expect("list before");
    let before_count = before.len();

    let op = branch_op("OP-100", "R000", "ai-draft");
    let outcome = apply_operation(&mut store, "test-doc", op).expect("branch apply");
    assert_eq!(outcome.revision.as_str(), "R001");

    // Branch name persisted to the side-table.
    let branch = crud::get_branch(store.conn(), "test-doc", "R001")
        .expect("get_branch")
        .expect("branch row present");
    assert_eq!(branch, "ai-draft");

    // Nodes unchanged (Branch is a label, not a mutation).
    let after = crud::list_nodes(store.conn(), "test-doc").expect("list after");
    assert_eq!(after.len(), before_count);
}

#[test]
fn branch_rejects_empty_name() {
    let mut store = setup();
    let op = branch_op("OP-101", "R000", "");
    let err = apply_operation(&mut store, "test-doc", op).expect_err("empty name rejected");
    let msg = format!("{err}");
    assert!(
        msg.contains("branch name"),
        "unexpected error: {msg}"
    );
}

#[test]
fn branch_rejects_main_name() {
    let mut store = setup();
    let op = branch_op("OP-102", "R000", "main");
    let err = apply_operation(&mut store, "test-doc", op).expect_err("main rejected");
    let msg = format!("{err}");
    assert!(
        msg.contains("not 'main'"),
        "unexpected error: {msg}"
    );
}

#[test]
fn branch_resolves_name_from_reason_when_attributes_missing() {
    // Fallback path: when `patch.attributes["branch"]` is absent, the engine
    // falls back to `reason` so existing callers can keep using it.
    let mut store = setup();
    let op = Operation {
        id: OpId::new("OP-103"),
        op_type: OperationType::Branch,
        target: None,
        expected_revision: RevisionId::new("R000"),
        target_revision: None,
        targets: vec![],
        actor: Provenance::human(Some("alice".into())),
        patch: None,
        reason: Some("experimental".into()),
    };
    let outcome = apply_operation(&mut store, "test-doc", op).expect("branch apply");
    let branch = crud::get_branch(store.conn(), "test-doc", outcome.revision.as_str())
        .expect("get_branch")
        .expect("branch row present");
    assert_eq!(branch, "experimental");
}