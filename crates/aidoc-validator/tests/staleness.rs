//! CodeRef staleness E2E (spec §37) — hermetic, uses a fake resolver so no
//! `git` binary or checkout is needed.
//!   cargo test -p aidoc-validator --test staleness

use std::collections::HashMap;

use aidoc_model::{Document, Node, NodeId, NodeKind, OpId, Revision, RevisionId};
use aidoc_storage::{AnyhowErr, Store, crud};
use aidoc_validator::{
    CodeState, GitResolver, ValidationCategory, default_validators_with_code_resolver,
    validate_with,
};

fn doc_id() -> &'static str {
    "staleness-test"
}

fn setup() -> Store {
    let mut store = Store::open_memory().expect("open memory");
    let doc = Document::new(doc_id(), "Staleness Test", NodeId::from_validated("root"));
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
            root.content = "Staleness Test".into();
            crud::insert_node(tx, doc_id(), &root)?;
            crud::insert_revision(tx, doc_id(), &rev, true)?;
            Ok::<_, AnyhowErr>(())
        })
        .expect("seed");
    store
}

fn seed_code_ref(store: &mut Store, id: &str, file: &str, commit: Option<&str>) {
    let mut node = Node::new(NodeId::from_validated(id), NodeKind::CodeRef);
    node.content = "see code".into();
    if !file.is_empty() {
        node.attributes.insert("file".into(), file.into());
    }
    if let Some(c) = commit {
        node.attributes.insert("commit".into(), c.into());
    }
    store
        .tx::<_, _, AnyhowErr>(|tx| {
            crud::insert_node(tx, doc_id(), &node).expect("insert");
            Ok::<_, AnyhowErr>(())
        })
        .expect("seed code-ref");
}

/// Maps file → CodeState; anything unlisted reads as Unknown.
struct FakeResolver(HashMap<String, CodeState>);

impl GitResolver for FakeResolver {
    fn state(&self, file: &str) -> CodeState {
        self.0.get(file).cloned().unwrap_or(CodeState::Unknown)
    }
}

fn validate_with_fake(store: &Store, states: &[(&str, CodeState)]) -> Vec<String> {
    let map = states
        .iter()
        .map(|(f, s)| (f.to_string(), s.clone()))
        .collect();
    let validators = default_validators_with_code_resolver(Box::new(FakeResolver(map)));
    let report = validate_with(store, doc_id(), &validators).expect("validate");
    report
        .by_category(ValidationCategory::CodeRef)
        .into_iter()
        .map(|f| f.message.clone())
        .collect()
}

#[test]
fn stale_ref_is_reported() {
    let mut store = setup();
    seed_code_ref(&mut store, "order-service", "src/order.ts", Some("aaaaaaa"));
    let findings = validate_with_fake(
        &store,
        &[("src/order.ts", CodeState::AtCommit("bbbbbbb".into()))],
    );
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert!(findings[0].contains("stale"), "{}", findings[0]);
    assert!(findings[0].contains("order-service"), "{}", findings[0]);
}

#[test]
fn synced_ref_is_clean() {
    let mut store = setup();
    seed_code_ref(&mut store, "order-service", "src/order.ts", Some("aaaaaaa"));
    let findings = validate_with_fake(
        &store,
        &[("src/order.ts", CodeState::AtCommit("aaaaaaa".into()))],
    );
    assert!(
        findings.is_empty(),
        "synced ref should not report: {findings:?}"
    );
}

#[test]
fn conflicted_ref_is_reported() {
    let mut store = setup();
    seed_code_ref(&mut store, "order-service", "src/order.ts", Some("aaaaaaa"));
    let findings = validate_with_fake(&store, &[("src/order.ts", CodeState::Conflicted)]);
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert!(findings[0].contains("conflict"), "{}", findings[0]);
}

#[test]
fn missing_commit_is_unknown_and_clean() {
    let mut store = setup();
    // No commit recorded → Unknown → not actionable → no finding.
    seed_code_ref(&mut store, "order-service", "src/order.ts", None);
    let findings = validate_with_fake(
        &store,
        &[("src/order.ts", CodeState::AtCommit("bbbbbbb".into()))],
    );
    assert!(
        findings.is_empty(),
        "unknown should not report: {findings:?}"
    );
}

#[test]
fn structural_check_still_fires_without_file() {
    let mut store = setup();
    seed_code_ref(&mut store, "bad-ref", "", Some("aaaaaaa"));
    let findings = validate_with_fake(&store, &[]);
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert!(
        findings[0].contains("missing file attribute"),
        "{}",
        findings[0]
    );
}
