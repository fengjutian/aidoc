//! Extensibility test: a custom `Validator` can be registered into the
//! validation pipeline, proving new checks can be added without touching
//! `validate()` itself.

use aidoc_model::{Document, Node, NodeId, NodeKind, OpId, Revision, RevisionId};
use aidoc_storage::{AnyhowErr, Store, crud};
use aidoc_validator::{Finding, ValidationCategory, ValidationError, Validator, validate_with};

fn doc_id() -> &'static str {
    "custom-val-test"
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

/// A validator that always emits a synthetic finding in the `Identity`
/// category, regardless of the document state.
struct AlwaysIdentityWarning;

impl Validator for AlwaysIdentityWarning {
    fn category(&self) -> ValidationCategory {
        ValidationCategory::Identity
    }
    fn check(&self, _store: &Store, _doc_id: &str) -> Result<Vec<Finding>, ValidationError> {
        Ok(vec![Finding::new(
            ValidationCategory::Identity,
            "custom validator was invoked",
        )])
    }
}

#[test]
fn custom_validator_finding_appears_in_report() {
    let store = setup();

    // Compose: all built-in validators + one custom one.
    let mut validators = aidoc_validator::default_validators();
    validators.push(Box::new(AlwaysIdentityWarning));

    let report = validate_with(&store, doc_id(), &validators).expect("validate");

    let identity = report.by_category(ValidationCategory::Identity);
    assert!(
        identity
            .iter()
            .any(|f| f.message == "custom validator was invoked"),
        "custom finding should appear in identity category, got {:?}",
        identity
    );
}

#[test]
fn custom_validator_composes_with_defaults() {
    let store = setup();

    // Run ONLY the custom validator — built-in checks should not fire.
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(AlwaysIdentityWarning)];

    let report = validate_with(&store, doc_id(), &validators).expect("validate");

    assert_eq!(report.total_errors(), 1, "only the custom finding expected");
    assert!(!report.is_clean());
}
