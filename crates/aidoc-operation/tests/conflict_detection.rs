//! Conflict detection integration tests (spec §25-§26, §36).
//!
//! Covers every conflict class the engine can raise:
//!   * REVISION_CONFLICT — stale `expected_revision` (§25)
//!   * CONTENT_CONFLICT  — `expected_hash` mismatch (§26)
//!   * NODE_CONFLICT     — target node missing
//!   * RELATION_CONFLICT — duplicate link / unlink of an absent relation
//!   * STRUCTURE_CONFLICT— move that would create a parent cycle

use aidoc_model::{
    Document, Node, NodeId, NodeKind, OpId, Operation, OperationType, Patch, Provenance, Revision,
    RevisionId,
};
use aidoc_operation::{ApplyError, Conflict, ConflictKind, apply_operation};
use aidoc_storage::{AnyhowErr, Store, crud};
use indexmap::IndexMap;

fn doc_id() -> &'static str {
    "conflict-doc"
}

fn setup() -> Store {
    let mut store = Store::open_memory().expect("open memory");
    let doc = Document::new(doc_id(), "Conflict Doc", NodeId::from_validated("root"));
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
            root.content = "Conflict Doc".into();
            crud::insert_node(tx, doc_id(), &root)?;
            crud::insert_revision(tx, doc_id(), &rev, true)?;
            Ok::<_, AnyhowErr>(())
        })
        .expect("seed");
    store
}

fn head(store: &Store) -> String {
    crud::head_revision(store.conn(), doc_id())
        .expect("head")
        .expect("has head")
}

fn create_op(id: &str, target: &str, expected_rev: &str, content: &str) -> Operation {
    Operation {
        id: OpId::new(id),
        op_type: OperationType::Create,
        target: Some(NodeId::from_validated(target)),
        expected_revision: RevisionId::new(expected_rev),
        expected_hash: None,
        target_revision: None,
        targets: vec![],
        actor: Provenance::human(Some("test".into())),
        patch: Some(Patch {
                kind: None,
                position: None,
            content: Some(content.into()),
            ..Default::default()
        }),
        reason: None,
    }
}

fn update_op(
    id: &str,
    target: &str,
    expected_rev: &str,
    content: &str,
    expected_hash: Option<String>,
) -> Operation {
    Operation {
        id: OpId::new(id),
        op_type: OperationType::Update,
        target: Some(NodeId::from_validated(target)),
        expected_revision: RevisionId::new(expected_rev),
        expected_hash,
        target_revision: None,
        targets: vec![],
        actor: Provenance::human(Some("test".into())),
        patch: Some(Patch {
                kind: None,
                position: None,
            content: Some(content.into()),
            ..Default::default()
        }),
        reason: None,
    }
}

#[test]
fn update_cannot_reparent_under_itself() {
    let mut store = setup();
    let created = apply_operation(&mut store, doc_id(), create_op("OP-1", "child", "R000", "text"))
        .expect("create child");
    let mut patch = Patch::default();
    patch.attributes.insert("parent".into(), "child".into());
    let mut op = update_op("OP-2", "child", created.revision.as_str(), "text", None);
    op.patch = Some(patch);
    let err = apply_operation(&mut store, doc_id(), op).expect_err("self-parent must conflict");
    assert!(matches!(err, ApplyError::Conflict(_)));
}

fn link_op(
    id: &str,
    op_type: OperationType,
    src: &str,
    dst: &str,
    expected_rev: &str,
) -> Operation {
    Operation {
        id: OpId::new(id),
        op_type,
        target: Some(NodeId::from_validated(dst)),
        expected_revision: RevisionId::new(expected_rev),
        expected_hash: None,
        target_revision: None,
        targets: vec![NodeId::from_validated(src)],
        actor: Provenance::human(Some("test".into())),
        patch: None,
        reason: None,
    }
}

fn assert_conflict(err: ApplyError, kind: ConflictKind) -> Conflict {
    match err {
        ApplyError::Conflict(c) => {
            assert_eq!(c.kind, kind, "wrong conflict kind: {c}");
            assert_eq!(c.to_json()["status"], "conflict");
            assert_eq!(c.to_json()["kind"], kind.as_str());
            c
        }
        other => panic!("expected Conflict({kind}), got: {other}"),
    }
}

#[test]
fn revision_conflict_is_structured() {
    let mut store = setup();
    let h = head(&store);
    apply_operation(
        &mut store,
        doc_id(),
        create_op("OP-001", "database", &h, "v1"),
    )
    .expect("create");

    // Stale expected_revision (R000) after head advanced.
    let err = apply_operation(
        &mut store,
        doc_id(),
        update_op("OP-002", "database", "R000", "v2", None),
    )
    .expect_err("stale revision must conflict");
    let c = assert_conflict(err, ConflictKind::Revision);
    assert_eq!(c.expected.as_deref(), Some("R000"));
    assert_ne!(c.actual.as_deref(), Some("R000"));
    assert_eq!(c.to_json()["expected_revision"], "R000");
}

#[test]
fn content_conflict_on_hash_mismatch() {
    let mut store = setup();
    let h = head(&store);
    apply_operation(
        &mut store,
        doc_id(),
        create_op("OP-001", "database", &h, "v1"),
    )
    .expect("create");

    let actual =
        crud::get_content_hash(store.conn(), doc_id(), &NodeId::from_validated("database"))
            .expect("hash")
            .expect("present");
    assert!(actual.starts_with("sha256:"), "unexpected hash: {actual}");
    let h2 = head(&store);

    // Wrong expected_hash → CONTENT_CONFLICT, node untouched.
    let err = apply_operation(
        &mut store,
        doc_id(),
        update_op(
            "OP-002",
            "database",
            &h2,
            "v2",
            Some("sha256:deadbeef".into()),
        ),
    )
    .expect_err("hash mismatch must conflict");
    let c = assert_conflict(err, ConflictKind::Content);
    assert_eq!(c.node.as_deref(), Some("database"));
    assert_eq!(c.to_json()["expected_hash"], "sha256:deadbeef");
    assert_eq!(c.to_json()["actual_hash"], actual.as_str());

    // Correct expected_hash → applies cleanly.
    let h3 = head(&store);
    apply_operation(
        &mut store,
        doc_id(),
        update_op("OP-003", "database", &h3, "v2", Some(actual.clone())),
    )
    .expect("matching hash applies");
    let node = crud::get_node(store.conn(), doc_id(), &NodeId::from_validated("database"))
        .expect("get")
        .expect("exists");
    assert_eq!(node.content, "v2");
}

#[test]
fn node_conflict_on_missing_target() {
    let mut store = setup();
    let h = head(&store);
    let err = apply_operation(
        &mut store,
        doc_id(),
        update_op("OP-001", "ghost", &h, "nope", None),
    )
    .expect_err("missing target must conflict");
    let c = assert_conflict(err, ConflictKind::Node);
    assert_eq!(c.node.as_deref(), Some("ghost"));
}

#[test]
fn relation_conflict_on_duplicate_link_and_absent_unlink() {
    let mut store = setup();
    let h = head(&store);
    apply_operation(&mut store, doc_id(), create_op("OP-001", "a", &h, "A")).expect("create a");
    let h = head(&store);
    apply_operation(&mut store, doc_id(), create_op("OP-002", "b", &h, "B")).expect("create b");

    // First link a→b succeeds.
    let h = head(&store);
    apply_operation(
        &mut store,
        doc_id(),
        link_op("OP-003", OperationType::Link, "a", "b", &h),
    )
    .expect("link");

    // Duplicate link → RELATION_CONFLICT.
    let h = head(&store);
    let err = apply_operation(
        &mut store,
        doc_id(),
        link_op("OP-004", OperationType::Link, "a", "b", &h),
    )
    .expect_err("duplicate link must conflict");
    assert_conflict(err, ConflictKind::Relation);

    // Unlink succeeds once...
    let h = head(&store);
    apply_operation(
        &mut store,
        doc_id(),
        link_op("OP-005", OperationType::Unlink, "a", "b", &h),
    )
    .expect("unlink");

    // ...then conflicts the second time.
    let h = head(&store);
    let err = apply_operation(
        &mut store,
        doc_id(),
        link_op("OP-006", OperationType::Unlink, "a", "b", &h),
    )
    .expect_err("absent unlink must conflict");
    assert_conflict(err, ConflictKind::Relation);
}

#[test]
fn structure_conflict_on_move_cycle() {
    let mut store = setup();
    // Seed a hierarchy directly: root → parent → child.
    store
        .tx::<_, _, AnyhowErr>(|tx| {
            let mut parent = Node::new(NodeId::from_validated("parent"), NodeKind::Section);
            parent.parent = Some(NodeId::from_validated("root"));
            parent.content = "parent".into();
            crud::insert_node(tx, doc_id(), &parent)?;
            let mut child = Node::new(NodeId::from_validated("child"), NodeKind::Section);
            child.parent = Some(NodeId::from_validated("parent"));
            child.content = "child".into();
            crud::insert_node(tx, doc_id(), &child)?;
            Ok::<_, AnyhowErr>(())
        })
        .expect("seed hierarchy");

    // Move `parent` under `child` (its own descendant) → STRUCTURE_CONFLICT.
    let h = head(&store);
    let mut attrs = IndexMap::new();
    attrs.insert("parent".into(), "child".into());
    let op = Operation {
        id: OpId::new("OP-010"),
        op_type: OperationType::Move,
        target: Some(NodeId::from_validated("parent")),
        expected_revision: RevisionId::new(h),
        expected_hash: None,
        target_revision: None,
        targets: vec![],
        actor: Provenance::human(Some("test".into())),
        patch: Some(Patch {
                kind: None,
                position: None,
            attributes: attrs,
            ..Default::default()
        }),
        reason: None,
    };
    let err = apply_operation(&mut store, doc_id(), op).expect_err("cycle must conflict");
    let c = assert_conflict(err, ConflictKind::Structure);
    assert_eq!(c.node.as_deref(), Some("parent"));
}
