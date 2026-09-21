use aidoc_model::{
    Document, Node, NodeId, NodeKind, OpId, Operation, OperationType, Patch, Provenance, Revision,
    RevisionId,
};
use aidoc_operation::{BranchError, apply_operation, checkout_branch, merge_branch};
use aidoc_storage::{AnyhowErr, Store, crud};

const DOC: &str = "merge-doc";

fn setup() -> Store {
    let mut store = Store::open_memory().unwrap();
    store
        .tx::<_, _, AnyhowErr>(|tx| {
            crud::upsert_document(
                tx,
                &Document::new(DOC, "Merge", NodeId::from_validated("root")),
            )?;
            let root = Node::new(NodeId::from_validated("root"), NodeKind::Section);
            crud::insert_node(tx, DOC, &root)?;
            for id in ["a", "b"] {
                let mut node = Node::new(NodeId::from_validated(id), NodeKind::Paragraph);
                node.parent = Some(NodeId::from_validated("root"));
                node.content = "base".into();
                crud::insert_node(tx, DOC, &node)?;
            }
            crud::insert_revision(
                tx,
                DOC,
                &Revision {
                    id: RevisionId::new("R000"),
                    parent: None,
                    operation: OpId::new("OP-0"),
                    created_at: chrono::Utc::now(),
                    message: None,
                    branch: None,
                },
                true,
            )?;
            let nodes = crud::list_nodes(tx, DOC)?;
            crud::save_snapshot(tx, DOC, "R000", &nodes)?;
            Ok::<_, AnyhowErr>(())
        })
        .unwrap();
    store
}

fn op(
    kind: OperationType,
    target: Option<&str>,
    head: &str,
    id: &str,
    content: Option<&str>,
    branch: Option<&str>,
) -> Operation {
    let mut patch = Patch::default();
    patch.content = content.map(str::to_owned);
    if let Some(branch) = branch {
        patch.attributes.insert("branch".into(), branch.into());
    }
    Operation {
        id: OpId::new(id),
        op_type: kind,
        target: target.map(NodeId::from_validated),
        expected_revision: RevisionId::new(head),
        expected_hash: None,
        target_revision: None,
        targets: vec![],
        actor: Provenance::human(None),
        patch: Some(patch),
        reason: None,
    }
}

#[test]
fn merges_disjoint_node_edits_and_records_two_parents() {
    let mut store = setup();
    let branch = apply_operation(
        &mut store,
        DOC,
        op(
            OperationType::Branch,
            None,
            "R000",
            "OP-1",
            None,
            Some("draft"),
        ),
    )
    .unwrap();
    let tip = apply_operation(
        &mut store,
        DOC,
        op(
            OperationType::Update,
            Some("a"),
            branch.revision.as_str(),
            "OP-2",
            Some("draft edit"),
            None,
        ),
    )
    .unwrap();
    assert_eq!(
        crud::get_branch(store.conn(), DOC, tip.revision.as_str())
            .unwrap()
            .as_deref(),
        Some("draft")
    );
    checkout_branch(&mut store, DOC, "main").unwrap();
    let main = apply_operation(
        &mut store,
        DOC,
        op(
            OperationType::Update,
            Some("b"),
            "R000",
            "OP-3",
            Some("main edit"),
            None,
        ),
    )
    .unwrap();
    let merged = merge_branch(&mut store, DOC, "draft", None).unwrap();
    assert_eq!(
        crud::list_parents(store.conn(), DOC, merged.as_str()).unwrap(),
        vec![main.revision.as_str(), tip.revision.as_str()]
    );
    assert_eq!(
        crud::get_node(store.conn(), DOC, &NodeId::from_validated("a"))
            .unwrap()
            .unwrap()
            .content,
        "draft edit"
    );
    assert_eq!(
        crud::get_node(store.conn(), DOC, &NodeId::from_validated("b"))
            .unwrap()
            .unwrap()
            .content,
        "main edit"
    );
}

#[test]
fn conflicting_edits_leave_head_unchanged() {
    let mut store = setup();
    let branch = apply_operation(
        &mut store,
        DOC,
        op(
            OperationType::Branch,
            None,
            "R000",
            "OP-1",
            None,
            Some("draft"),
        ),
    )
    .unwrap();
    apply_operation(
        &mut store,
        DOC,
        op(
            OperationType::Update,
            Some("a"),
            branch.revision.as_str(),
            "OP-2",
            Some("draft edit"),
            None,
        ),
    )
    .unwrap();
    checkout_branch(&mut store, DOC, "main").unwrap();
    let main = apply_operation(
        &mut store,
        DOC,
        op(
            OperationType::Update,
            Some("a"),
            "R000",
            "OP-3",
            Some("main edit"),
            None,
        ),
    )
    .unwrap();
    assert!(
        matches!(merge_branch(&mut store, DOC, "draft", None), Err(BranchError::Conflicts(ids)) if ids == vec!["a"])
    );
    assert_eq!(
        crud::head_revision(store.conn(), DOC).unwrap().unwrap(),
        main.revision.as_str()
    );
}
