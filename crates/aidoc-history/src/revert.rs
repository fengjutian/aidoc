//! Revert implementation.

use chrono::Utc;
use rusqlite::Transaction;
use thiserror::Error;

use aidoc_model::{
    Change, ChangeType, HashRef, Node, Operation, OperationType, Provenance,
    id::{OpId, RevisionId, sha256_hex},
};

use aidoc_storage::{Store, crud};

#[derive(Debug, Error)]
pub enum RevertError {
    #[error(transparent)]
    Store(#[from] aidoc_storage::StoreError),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("cannot revert to head revision")]
    RevertToHead,

    #[error("revert target not found: {0}")]
    TargetNotFound(String),
}

#[derive(Debug, Clone)]
pub struct RevertOutcome {
    pub new_revision: RevisionId,
    pub op_id: OpId,
    pub target_revision: RevisionId,
    pub nodes_touched: Set<String>,
}

/// Minimal `Set<String>`-like wrapper, just so we don't pull in a new dep.
pub type Set<T> = std::collections::BTreeSet<T>;

/// Revert the document to a previous revision. Always produces a new revision
/// (MUST 4 from spec §54).
pub fn revert_to(
    store: &mut Store,
    doc_id: &str,
    target_revision: RevisionId,
    reason: Option<String>,
) -> Result<RevertOutcome, RevertError> {
    // Confirm target exists.
    let target_rev = crud::get_revision(store.conn(), doc_id, target_revision.as_str())?
        .ok_or_else(|| RevertError::TargetNotFound(target_revision.as_str().into()))?;

    // current head becomes the parent of the new revision.
    let head = crud::head_revision(store.conn(), doc_id)?.ok_or(RevertError::RevertToHead)?;
    if head == target_revision.as_str() {
        return Err(RevertError::RevertToHead);
    }

    // Build a synthetic Operation describing this revert.
    let seq = crud::max_revision_seq(store.conn(), doc_id)?;
    let new_rev = RevisionId::from_sequence(seq);
    let op_id = OpId::new(format!("OP-RVT-{:03}", seq));

    let op = Operation {
        id: op_id.clone(),
        op_type: OperationType::Revert,
        target: None,
        expected_revision: RevisionId::new(head.clone()),
        expected_hash: None,
        target_revision: Some(target_rev.id.clone()),
        targets: Vec::new(),
        actor: Provenance::human(None),
        patch: None,
        reason: reason.clone(),
    };

    // Snapshot target nodes (state at target_revision).
    let target_nodes = snapshot_at_revision(store, doc_id, &target_rev.id)?;

    store.tx(|tx| {
        // 1. Overwrite live nodes with target snapshot.
        // Delete all current nodes for the doc.
        tx.execute(
            "DELETE FROM nodes WHERE doc_id = ?1",
            rusqlite::params![doc_id],
        )?;
        for n in &target_nodes {
            crud::insert_node(tx, doc_id, n)?;
        }

        // 2. Emit a Revert change for each node touched.
        for n in &target_nodes {
            let after = sha256_hex(n.content.as_bytes());
            let ch = Change {
                id: format!("CH-{}-{}", new_rev.as_str(), n.id.as_str()),
                revision: new_rev.clone(),
                node: n.id.clone(),
                change_type: ChangeType::Revert,
                before: None,
                after: Some(HashRef { hash: after }),
                summary: Some(format!("revert to {}", target_rev.id.as_str())),
            };
            crud::insert_change(tx, doc_id, &ch)?;
        }

        // 3. Record Operation + new Revision.
        let created_at = Utc::now();
        crud::insert_operation(tx, doc_id, &op, created_at)?;

        crud::advance_head(tx, doc_id, new_rev.as_str())?;
        crud::insert_revision(
            tx,
            doc_id,
            &aidoc_model::Revision {
                id: new_rev.clone(),
                parent: Some(RevisionId::new(head.clone())),
                operation: op_id.clone(),
                created_at,
                message: op.reason.clone(),
                branch: None,
            },
            true,
        )?;

        Ok::<(), RevertError>(())
    })?;

    let nodes_touched: Set<String> = target_nodes.iter().map(|n| n.id.as_str().into()).collect();

    Ok(RevertOutcome {
        new_revision: new_rev,
        op_id,
        target_revision: target_rev.id,
        nodes_touched,
    })
}

/// Best-effort snapshot of node state at a given revision.
///
/// v0.1 uses eager snapshots: every applied op persists the full live-node
/// state to the `snapshots` table. Revert reads that blob.
fn snapshot_at_revision(
    store: &Store,
    doc_id: &str,
    rev_id: &RevisionId,
) -> Result<Vec<Node>, RevertError> {
    match crud::load_snapshot(store.conn(), doc_id, rev_id.as_str()).map_err(RevertError::Store)? {
        Some(nodes) => Ok(nodes),
        None => Err(RevertError::TargetNotFound(format!(
            "no snapshot for {}",
            rev_id.as_str()
        ))),
    }
}

// silence unused warning for `Transaction` alias
#[allow(dead_code)]
fn _ensure_used(_: &Transaction<'_>) {}
