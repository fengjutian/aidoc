//! Apply an Operation to the store, atomically producing a new Revision.
//!
//! Orchestration only: the per-op mutation logic lives in [`crate::handler`]
//! behind the [`OperationHandler`] trait. This module owns the transaction,
//! optimistic-concurrency check, revision bookkeeping and snapshot — the parts
//! that are identical for every op.

use chrono::Utc;
use thiserror::Error;

use aidoc_model::{
    Operation, OperationType,
    id::{AIDocError, OpId, RevisionId},
};

use aidoc_storage::{Store, crud};

use crate::handler::{ApplyContext, HandlerRegistry};

#[derive(Debug, Error)]
pub enum ApplyError {
    #[error(transparent)]
    Model(#[from] AIDocError),

    #[error(transparent)]
    Store(#[from] aidoc_storage::StoreError),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("operation requires a target node: {0}")]
    MissingTarget(&'static str),

    #[error("operation requires a patch")]
    MissingPatch,

    #[error("split requires targets")]
    MissingSplitTargets,

    #[error("invalid operation: {0}")]
    Invalid(String),

    #[error("unknown operation: {0}")]
    UnknownOperation(String),
}

#[derive(Debug, Clone)]
pub struct ApplyOutcome {
    pub revision: RevisionId,
    pub op_id: OpId,
}

/// Apply a single forward op under one transaction, using the built-in
/// [`HandlerRegistry`].
pub fn apply_operation(
    store: &mut Store,
    doc_id: &str,
    op: Operation,
) -> Result<ApplyOutcome, ApplyError> {
    apply_with_registry(store, doc_id, op, &HandlerRegistry::default())
}

/// Apply a single forward op under one transaction, dispatching the mutation to
/// the handler registered for `op.op_type` in `registry`.
///
/// This is the seam that lets a caller (or a test) add / override ops without
/// touching the engine. Ops with no registered handler — including `revert`,
/// which `aidoc-history` orchestrates — yield [`ApplyError::UnknownOperation`],
/// matching the pre-refactor `match` fallthrough.
pub fn apply_with_registry(
    store: &mut Store,
    doc_id: &str,
    op: Operation,
    registry: &HandlerRegistry,
) -> Result<ApplyOutcome, ApplyError> {
    // 1. Optimistic concurrency check (read-only, no tx needed).
    crate::check::check_revision(store, doc_id, &op)
        .map_err(|e| ApplyError::Store(aidoc_storage::StoreError::Integrity(e.to_string())))?;

    // 2. Materialize the new revision id BEFORE we enter the tx.
    let next_seq = crud::max_revision_seq(store.conn(), doc_id).map_err(ApplyError::Store)?;
    let new_rev = RevisionId::from_sequence(next_seq);
    let op_id = op.id.clone();
    // Resolve the branch name (if this op is a Branch) before we go into the
    // transaction so we can attach it to the new Revision row.
    let resolved_branch = match op.op_type {
        OperationType::Branch => op
            .patch
            .as_ref()
            .and_then(|p| p.attributes.get("branch").cloned())
            .or_else(|| op.reason.clone()),
        _ => None,
    };

    // 3. Apply inside a single SQLite transaction.
    store.tx(|tx| {
        // 3a. Dispatch the mutation to the registered handler.
        let mut ctx = ApplyContext {
            tx,
            doc_id,
            new_rev: &new_rev,
            op: &op,
        };
        match registry.get(op.op_type) {
            Some(handler) => handler.apply(&mut ctx)?,
            None => return Err(ApplyError::UnknownOperation(op.op_type.as_str().to_owned())),
        }

        // 4. Write Operation + Revision + advance head.
        let created_at = Utc::now();
        crud::insert_operation(tx, doc_id, &op, created_at)?;

        let new_revision = aidoc_model::Revision {
            id: new_rev.clone(),
            parent: Some(op.expected_revision.clone()),
            operation: op_id.clone(),
            created_at,
            message: op.reason.clone(),
            branch: resolved_branch.clone(),
        };
        crud::advance_head(tx, doc_id, new_rev.as_str())?;
        crud::insert_revision(tx, doc_id, &new_revision, true)?;

        // Snapshot live node state so a future revert can replay it byte-for-byte.
        let live = crud::list_nodes(tx, doc_id).map_err(ApplyError::Store)?;
        crud::save_snapshot(tx, doc_id, new_rev.as_str(), &live).map_err(ApplyError::Store)?;

        Ok(())
    })?;

    Ok(ApplyOutcome {
        revision: new_rev,
        op_id,
    })
}

// `crud::max_revision_seq` owns the R### math now; see aidoc-storage.
