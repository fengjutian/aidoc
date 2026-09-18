//! Integration test for the full create → update → revert loop.
//!
//! Hits every MUST rule from spec §54:
//!   1. Stable node ID
//!   2. Modify through Operation
//!   3. Each modification produces a Revision
//!   4. Revert creates a new Revision (history is immutable)

use aidoc_history::revert_to;
use aidoc_model::{
    Document, Node, NodeId, NodeKind, OpId, Operation, OperationType, Patch, Provenance, Revision,
    RevisionId,
};
use aidoc_operation::apply_operation;
use aidoc_storage::{AnyhowErr, Store, crud};

fn doc_id() -> &'static str {
    "test-doc"
}

fn setup() -> Store {
    let mut store = Store::open_memory().expect("open memory");
    let doc = Document::new(doc_id(), "Test Doc", NodeId::from_validated("root"));
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
            crud::insert_node(tx, doc_id(), &root)?;
            crud::insert_revision(tx, doc_id(), &rev, true)?;
            Ok::<_, AnyhowErr>(())
        })
        .expect("seed");
    store
}

#[test]
fn create_update_revert_loop() {
    let mut store = setup();

    // 1. CREATE "database" with content v1.
    let create_op = Operation {
        id: OpId::new("OP-001"),
        op_type: OperationType::Create,
        target: Some(NodeId::from_validated("database")),
        expected_revision: RevisionId::new("R000"),
        expected_hash: None,
        target_revision: None,
        targets: vec![],
        actor: Provenance::human(Some("test".into())),
        patch: Some(Patch {
                kind: None,
                position: None,
            content: Some("v1: 系统使用 MySQL 8.0。".into()),
            ..Default::default()
        }),
        reason: Some("create".into()),
    };
    let out1 = apply_operation(&mut store, doc_id(), create_op).expect("create");
    assert!(
        out1.revision.as_str().starts_with("R00"),
        "got {}",
        out1.revision.as_str()
    );

    // 2. UPDATE with content v2.
    let update_op = Operation {
        id: OpId::new("OP-002"),
        op_type: OperationType::Update,
        target: Some(NodeId::from_validated("database")),
        expected_revision: out1.revision.clone(),
        expected_hash: None,
        target_revision: None,
        targets: vec![],
        actor: Provenance::ai("test-agent", Some("test-model".into())),
        patch: Some(Patch {
                kind: None,
                position: None,
            content: Some("v2: 系统使用 MySQL 8.4，包含分库分表。".into()),
            ..Default::default()
        }),
        reason: Some("update".into()),
    };
    let out2 = apply_operation(&mut store, doc_id(), update_op).expect("update");
    assert!(
        out2.revision.as_str() != out1.revision.as_str(),
        "update must produce a new revision"
    );

    // Live node should now have v2 content.
    let node = crud::get_node(store.conn(), doc_id(), &NodeId::from_validated("database"))
        .expect("get")
        .expect("exists");
    assert!(node.content.contains("MySQL 8.4"));

    // 3. REVERT to post-create revision.
    let outcome = revert_to(
        &mut store,
        doc_id(),
        out1.revision.clone(),
        Some("test revert".into()),
    )
    .expect("revert");
    assert_eq!(outcome.target_revision.as_str(), out1.revision.as_str());
    assert!(
        outcome.new_revision.as_str() != out1.revision.as_str()
            && outcome.new_revision.as_str() != out2.revision.as_str(),
        "revert must create a NEW revision, got {}",
        outcome.new_revision.as_str()
    );

    // After revert, live node should have v1 content again (MUST 4).
    let node = crud::get_node(store.conn(), doc_id(), &NodeId::from_validated("database"))
        .expect("get")
        .expect("exists");
    assert!(
        node.content.contains("MySQL 8.0"),
        "expected revert content, got: {}",
        node.content
    );
    assert!(!node.content.contains("8.4"));

    // 4. History is immutable: we now have at least 4 revisions (R000, create, update, revert).
    let revs = crud::list_revisions(store.conn(), doc_id()).expect("list");
    assert!(
        revs.len() >= 4,
        "expected >= 4 revisions, got {}",
        revs.len()
    );
    let head = crud::head_revision(store.conn(), doc_id())
        .expect("head")
        .expect("has head");
    assert_eq!(head, outcome.new_revision.as_str());

    // 5. Optimistic concurrency: trying to apply an op with stale
    //    expected_revision must fail.
    let stale_op = Operation {
        id: OpId::new("OP-999"),
        op_type: OperationType::Update,
        target: Some(NodeId::from_validated("database")),
        expected_revision: RevisionId::new("R000"), // stale!
        expected_hash: None,
        target_revision: None,
        targets: vec![],
        actor: Provenance::human(None),
        patch: Some(Patch {
                kind: None,
                position: None,
            content: Some("should not apply".into()),
            ..Default::default()
        }),
        reason: None,
    };
    let err = apply_operation(&mut store, doc_id(), stale_op).expect_err("must conflict");
    let msg = err.to_string();
    assert!(
        msg.contains("conflict") || msg.contains("Conflict"),
        "expected conflict error, got: {msg}"
    );
}
