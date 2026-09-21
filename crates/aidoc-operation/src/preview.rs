//! Dry-run path for [`Operation`]s (spec §25 / §26 / §36 preview support).
//!
//! [`preview_operation`] runs the same handler the real apply path uses, but
//! inside a SQLite savepoint that is rolled back before returning. The live
//! store is therefore never mutated. The function returns a [`PreviewReport`]
//! that captures:
//!
//! - the head revision this op would build on top of
//! - the next revision id the op would create
//! - a per-node diff (added / removed / modified) with before/after snapshots
//! - relation-level diff (added / removed)
//!
//! Preconditions still run through [`crate::check::check_all`], so a
//! `CONTENT_CONFLICT` or `STALE_REVISION` surfaces here exactly the way it
//! would during apply. The caller (UI, MCP server) can then either
//! accept-then-apply or reject the op.

use rusqlite::{Connection, Transaction};

use aidoc_model::{Node, NodeId, Operation, Relation, RevisionId};
use aidoc_storage::{Store, StoreError, crud};

use crate::apply::ApplyError;
use crate::check::{CheckError, check_all};
use crate::handler::{ApplyContext, HandlerRegistry};

/// One node-level entry in a [`PreviewReport`].
#[derive(Debug, Clone, PartialEq)]
pub enum NodeDiff {
    /// Node did not exist before this op and will exist after.
    Added {
        node: Node,
    },
    /// Node existed before and will be removed.
    Removed {
        node: Node,
    },
    /// Node existed before and is being changed.
    Modified {
        before: Node,
        after: Node,
    },
    Unchanged,
}

/// One relation-level entry in a [`PreviewReport`].
#[derive(Debug, Clone, PartialEq)]
pub enum RelationDiff {
    Added { relation: Relation },
    Removed { relation: Relation },
}

/// Whole-document preview for one [`Operation`].
#[derive(Debug, Clone)]
pub struct PreviewReport {
    /// The current head revision at the time of preview. The op would be
    /// applied on top of this; if the live state has advanced by the time
    /// the caller decides to actually apply, they should re-preview.
    pub current_revision: RevisionId,
    /// The revision id the op *would* create.
    pub next_revision: RevisionId,
    /// Per-node diff keyed by [`NodeId`].
    pub nodes: Vec<(NodeId, NodeDiff)>,
    /// Per-relation diff keyed by relation id.
    pub relations: Vec<(String, RelationDiff)>,
    /// A human-readable summary suitable for direct UI rendering.
    pub summary: String,
}

/// Snapshot of the live state at the moment preview was requested.
#[derive(Debug, Clone)]
struct Snapshot {
    nodes: Vec<Node>,
    relations: Vec<Relation>,
    head: RevisionId,
}

impl Snapshot {
    fn capture(conn: &Connection, doc_id: &str) -> Result<Self, StoreError> {
        Ok(Self {
            nodes: crud::list_nodes(conn, doc_id)?,
            relations: crud::list_relations(conn, doc_id)?,
            head: RevisionId::new(
                crud::head_revision(conn, doc_id)?.unwrap_or_else(|| "R000".into()),
            ),
        })
    }
}

/// Preview an operation without mutating the store.
///
/// Returns a [`PreviewReport`] describing what the op would change, or an
/// [`ApplyError`] (e.g. `Conflict`) matching what `apply_operation` would have
/// produced — the caller can render that error verbatim to the user.
pub fn preview_operation(
    store: &mut Store,
    doc_id: &str,
    op: &Operation,
) -> Result<PreviewReport, ApplyError> {
    preview_with_registry(store, doc_id, op, &HandlerRegistry::default())
}

/// Same as [`preview_operation`] with a custom handler registry.
pub fn preview_with_registry(
    store: &mut Store,
    doc_id: &str,
    op: &Operation,
    registry: &HandlerRegistry,
) -> Result<PreviewReport, ApplyError> {
    // 1. Read-only preconditions. A §36 conflict surfaces here the same way
    //    it would during apply, so the caller never sees an inconsistent
    //    preview.
    if let Err(e) = check_all(store, doc_id, op) {
        return Err(match e {
            CheckError::Conflict(c) => ApplyError::Conflict(c),
            other => ApplyError::Store(StoreError::Integrity(other.to_string())),
        });
    }

    // 2. Capture the "before" snapshot before touching anything.
    let before = Snapshot::capture(store.conn(), doc_id)?;

    // 3. Materialize the next revision id the op would receive.
    let next_seq = crud::max_revision_seq(store.conn(), doc_id).map_err(ApplyError::Store)?;
    let next_rev = RevisionId::from_sequence(next_seq);

    // 4. Run the handler inside an SQLite savepoint that we always roll back.
    //    Queries against the open transaction see the uncommitted state, so
    //    we can read the "after" snapshot before reverting.
    let conn = store.conn_mut();
    let tx = conn.transaction().map_err(ApplyError::Sqlite)?;
    let after = run_inside_savepoint(&tx, doc_id, &next_rev, op, registry)?;

    // 5. Drop `tx` without committing → SQLite rolls back the savepoint AND
    //    the outer transaction. The store is back to its pre-preview state.
    drop(tx);

    // 6. Diff before vs after.
    let report = diff_snapshots(&before, &after, &next_rev);
    Ok(report)
}

fn run_inside_savepoint(
    tx: &Transaction<'_>,
    doc_id: &str,
    next_rev: &RevisionId,
    op: &Operation,
    registry: &HandlerRegistry,
) -> Result<Snapshot, ApplyError> {
    tx.execute("SAVEPOINT pre_apply", [])
        .map_err(ApplyError::Sqlite)?;
    let apply_result: Result<Snapshot, ApplyError> = (|| {
        let mut ctx = ApplyContext {
            tx,
            doc_id,
            new_rev: next_rev,
            op,
        };
        match registry.get(op.op_type) {
            Some(handler) => handler.apply(&mut ctx)?,
            None => {
                return Err(ApplyError::UnknownOperation(op.op_type.as_str().to_owned()));
            }
        }
        Snapshot::capture(tx, doc_id).map_err(ApplyError::Store)
    })();
    // Always roll back, regardless of apply outcome. If apply itself failed
    // mid-way the savepoint still reverts the partial work.
    let rollback = tx.execute("ROLLBACK TO pre_apply", []);
    let release = tx.execute("RELEASE pre_apply", []);
    rollback?;
    release?;
    apply_result
}

fn diff_snapshots(
    before: &Snapshot,
    after: &Snapshot,
    next_rev: &RevisionId,
) -> PreviewReport {
    let mut node_diffs: Vec<(NodeId, NodeDiff)> = Vec::new();
    for n in &after.nodes {
        match before.nodes.iter().find(|b| b.id == n.id) {
            None => node_diffs.push((n.id.clone(), NodeDiff::Added { node: n.clone() })),
            Some(b) if b != n => node_diffs.push((
                n.id.clone(),
                NodeDiff::Modified {
                    before: b.clone(),
                    after: n.clone(),
                },
            )),
            Some(_) => {}
        }
    }
    for b in &before.nodes {
        if !after.nodes.iter().any(|a| a.id == b.id) {
            node_diffs.push((b.id.clone(), NodeDiff::Removed { node: b.clone() }));
        }
    }

    let mut rel_diffs: Vec<(String, RelationDiff)> = Vec::new();
    for r in &after.relations {
        if !before.relations.iter().any(|b| b.id == r.id) {
            rel_diffs.push((r.id.clone(), RelationDiff::Added { relation: r.clone() }));
        }
    }
    for b in &before.relations {
        if !after.relations.iter().any(|a| a.id == b.id) {
            rel_diffs.push((b.id.clone(), RelationDiff::Removed { relation: b.clone() }));
        }
    }

    let summary = build_summary(&node_diffs, &rel_diffs);

    PreviewReport {
        current_revision: before.head.clone(),
        next_revision: next_rev.clone(),
        nodes: node_diffs,
        relations: rel_diffs,
        summary,
    }
}

fn build_summary(nodes: &[(NodeId, NodeDiff)], rels: &[(String, RelationDiff)]) -> String {
    let added = nodes.iter().filter(|(_, d)| matches!(d, NodeDiff::Added { .. })).count();
    let removed = nodes.iter().filter(|(_, d)| matches!(d, NodeDiff::Removed { .. })).count();
    let modified = nodes.iter().filter(|(_, d)| matches!(d, NodeDiff::Modified { .. })).count();
    let rels_added = rels.iter().filter(|(_, d)| matches!(d, RelationDiff::Added { .. })).count();
    let rels_removed = rels
        .iter()
        .filter(|(_, d)| matches!(d, RelationDiff::Removed { .. }))
        .count();
    format!(
        "preview: +{added} nodes, -{removed} nodes, ~{modified} modified, \
         +{rels_added} relations, -{rels_removed} relations"
    )
}

#[doc(hidden)]
pub fn _export_for_macro() {}

#[cfg(test)]
mod tests {
    use super::*;
    use aidoc_model::{
        Actor, ActorKind, Node, NodeKind, Operation, OperationType, Patch, Provenance, RevisionId,
    };
    use aidoc_model::id::{NodeId, OpId};

    fn open_store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.db");
        let store = Store::open(&path).unwrap();
        // Seed a document + root node + R000 so ops have something to act on.
        let doc = aidoc_model::Document::new("d", "T", NodeId::from_validated("root"));
        crud::upsert_document(store.conn(), &doc).unwrap();
        let mut store = store;
        store
            .tx::<_, _, StoreError>(|tx| {
                let mut root = Node::new(NodeId::from_validated("root"), NodeKind::Section);
                root.content = "Hello".into();
                crud::insert_node(tx, "d", &root)?;
                let rev = aidoc_model::Revision {
                    id: RevisionId::new("R000"),
                    parent: None,
                    operation: OpId::new("OP-000"),
                    created_at: crud::now(),
                    message: Some("seed".into()),
                    branch: None,
                };
                crud::insert_revision(tx, "d", &rev, true)?;
                Ok(())
            })
            .unwrap();
        (dir, store)
    }

    fn human_prov() -> Provenance {
        Provenance::human(Some("preview-test".into()))
    }

    fn make_op(id: &str, kind: OperationType, target: Option<NodeId>, patch: Option<Patch>) -> Operation {
        Operation {
            id: OpId::new(id),
            op_type: kind,
            target,
            targets: Vec::new(),
            expected_revision: RevisionId::new("R000"),
            expected_hash: None,
            target_revision: None,
            actor: Provenance::Operation {
                actor: Actor {
                    kind: ActorKind::Human,
                    id: Some("tester".into()),
                    agent: None,
                    model: None,
                },
                task: None,
                reason: None,
                prompt: None,
                tool_calls: Vec::new(),
                temperature: None,
                input_refs: Default::default(),
                output: None,
                reasoning_summary: None,
            },
            patch,
            reason: None,
        }
    }

    #[test]
    fn preview_does_not_mutate_store() {
        let (_dir, mut store) = open_store();
        let op = make_op(
            "op-1",
            OperationType::Update,
            Some(NodeId::from_validated("root")),
            Some(Patch {
                content: Some("Goodbye".into()),
                ..Default::default()
            }),
        );
        let report = preview_operation(&mut store, "d", &op).expect("preview");
        assert_eq!(report.next_revision.as_str(), "R001");
        assert_eq!(report.summary, "preview: +0 nodes, -0 nodes, ~1 modified, +0 relations, -0 relations");
        // Live state must be untouched.
        let live: Vec<Node> = crud::list_nodes(store.conn(), "d").unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].content, "Hello");
        let head = crud::head_revision(store.conn(), "d").unwrap();
        assert_eq!(head.as_deref(), Some("R000"));
    }

    #[test]
    fn preview_reports_added_and_removed_nodes() {
        let (_dir, mut store) = open_store();

        // Create a new paragraph.
        let create = make_op(
            "op-c",
            OperationType::Create,
            Some(NodeId::from_validated("p1")),
            Some(Patch {
                content: Some("world".into()),
                kind: Some(NodeKind::Paragraph),
                position: Some(0),
                ..Default::default()
            }),
        );
        let r1 = preview_operation(&mut store, "d", &create).unwrap();
        assert!(matches!(r1.nodes.as_slice(), [(id, NodeDiff::Added { .. })] if id.as_str() == "p1"));

        // Delete the root node.
        let del = make_op(
            "op-d",
            OperationType::Delete,
            Some(NodeId::from_validated("root")),
            None,
        );
        let r2 = preview_operation(&mut store, "d", &del).unwrap();
        assert!(r2
            .nodes
            .iter()
            .any(|(id, d)| id.as_str() == "root" && matches!(d, NodeDiff::Removed { .. })));

        // After both previews the store still holds R000 / original content.
        assert_eq!(crud::head_revision(store.conn(), "d").unwrap().as_deref(), Some("R000"));
    }

    #[test]
    fn preview_propagates_conflict_errors() {
        let (_dir, mut store) = open_store();
        // Stale expected revision triggers §36.
        let mut op = make_op(
            "op-x",
            OperationType::Update,
            Some(NodeId::from_validated("root")),
            Some(Patch {
                content: Some("x".into()),
                ..Default::default()
            }),
        );
        op.expected_revision = RevisionId::new("R999");
        let err = preview_operation(&mut store, "d", &op).unwrap_err();
        assert!(matches!(err, ApplyError::Conflict(_)));
    }

    #[test]
    fn preview_link_and_unlink_diff_relations() {
        let (_dir, mut store) = open_store();

        // First, *apply* a real create op so p1 actually exists in the store.
        // (Preview's savepoint is rolled back, so previews alone don't persist
        // new nodes — we need a real apply here.)
        let create = make_op(
            "op-c",
            OperationType::Create,
            Some(NodeId::from_validated("p1")),
            Some(Patch {
                content: Some("hi".into()),
                kind: Some(NodeKind::Paragraph),
                position: Some(0),
                ..Default::default()
            }),
        );
        crate::apply_operation(&mut store, "d", create).unwrap();

        // Link op: targets=[p1], target=root, attributes.relation_kind = "depends-on".
        let mut link = make_op(
            "op-link",
            OperationType::Link,
            Some(NodeId::from_validated("root")),
            Some(Patch {
                attributes: {
                    let mut m = indexmap::IndexMap::new();
                    m.insert("relation_kind".into(), "depends-on".into());
                    m
                },
                ..Default::default()
            }),
        );
        // Move past R000 (which the apply above advanced).
        link.expected_revision = RevisionId::new("R001");
        link.targets = vec![NodeId::from_validated("p1")];
        let report = preview_operation(&mut store, "d", &link).unwrap();
        assert!(report
            .relations
            .iter()
            .any(|(_, d)| matches!(d, RelationDiff::Added { .. })));

        // Unlink preview should report removal — but only AFTER the link was
        // actually applied. Preview link doesn't persist anything.
        crate::apply_operation(&mut store, "d", link).unwrap();
        let mut unlink = make_op(
            "op-unlink",
            OperationType::Unlink,
            Some(NodeId::from_validated("root")),
            None,
        );
        unlink.expected_revision = RevisionId::new("R002");
        unlink.targets = vec![NodeId::from_validated("p1")];
        let report = preview_operation(&mut store, "d", &unlink).unwrap();
        assert!(report
            .relations
            .iter()
            .any(|(_, d)| matches!(d, RelationDiff::Removed { .. })));

        // Live state must remain as it was after the real apply.
        let live_rels = crud::list_relations(store.conn(), "d").unwrap();
        assert_eq!(live_rels.len(), 1, "link op was applied, should remain");
    }
}