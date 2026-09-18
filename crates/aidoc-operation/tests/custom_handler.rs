//! Extensibility test: a custom `OperationHandler` can be registered into the
//! `HandlerRegistry` to override a built-in op, proving new ops (or replacement
//! handlers) can be added without touching the engine's orchestration.

use aidoc_model::{
    Document, Node, NodeId, NodeKind, OpId, Operation, OperationType, Patch, Provenance, Revision,
    RevisionId,
};
use aidoc_operation::{
    ApplyContext, ApplyError, HandlerRegistry, OperationHandler, apply_with_registry,
};
use aidoc_storage::{AnyhowErr, Store, crud};

fn doc_id() -> &'static str {
    "handler-ext-test"
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

fn create_op() -> Operation {
    Operation {
        id: OpId::new("OP-001"),
        op_type: OperationType::Create,
        target: Some(NodeId::from_validated("new-node")),
        expected_revision: RevisionId::new("R000"),
        expected_hash: None,
        target_revision: None,
        targets: vec![],
        actor: Provenance::human(None),
        patch: Some(Patch {
                kind: None,
                position: None,
            content: Some("hello".into()),
            ..Default::default()
        }),
        reason: None,
    }
}

/// A no-op handler that replaces Create: it does nothing except mark
/// that it ran (via the revision bookkeeping the engine always produces).
struct NoopCreateHandler;

impl OperationHandler for NoopCreateHandler {
    fn op_type(&self) -> OperationType {
        OperationType::Create
    }
    fn apply(&self, _ctx: &mut ApplyContext) -> Result<(), ApplyError> {
        // Intentionally skip insert_node — proves the custom handler was
        // dispatched instead of the built-in CreateHandler.
        Ok(())
    }
}

#[test]
fn custom_handler_overrides_builtin_create() {
    let mut store = setup();

    let mut registry = HandlerRegistry::default();
    registry.register(Box::new(NoopCreateHandler));

    let outcome = apply_with_registry(&mut store, doc_id(), create_op(), &registry)
        .expect("noop handler should succeed");

    // The engine still produces a new revision (orchestration is unchanged).
    assert!(
        outcome.revision.as_str().starts_with("R00"),
        "got {}",
        outcome.revision.as_str()
    );

    // But the custom handler was a no-op — "new-node" must NOT exist.
    let node =
        crud::get_node(store.conn(), doc_id(), &NodeId::from_validated("new-node")).expect("get");
    assert!(
        node.is_none(),
        "custom noop handler should NOT have inserted the node, found: {:?}",
        node.map(|n| n.id.as_str().to_owned())
    );
}

#[test]
fn default_registry_applies_builtin_create() {
    let mut store = setup();

    let registry = HandlerRegistry::default();
    let outcome =
        apply_with_registry(&mut store, doc_id(), create_op(), &registry).expect("builtin create");

    assert!(
        outcome.revision.as_str().starts_with("R00"),
        "got {}",
        outcome.revision.as_str()
    );

    // Built-in handler actually inserted the node.
    let node = crud::get_node(store.conn(), doc_id(), &NodeId::from_validated("new-node"))
        .expect("get")
        .expect("node exists");
    assert_eq!(node.content, "hello");
}
