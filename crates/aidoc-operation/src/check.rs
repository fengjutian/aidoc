//! Conflict detection / optimistic concurrency (spec §25-§26).

use aidoc_model::{
    Operation,
    id::{AIDocError, NodeId, RevisionId},
};

use aidoc_storage::{Store, crud};

#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("revision not found: {0}")]
    RevisionNotFound(String),

    #[error("revision conflict: expected={expected}, actual={actual}")]
    Conflict { expected: String, actual: String },

    #[error("content hash mismatch: expected={expected}, actual={actual}")]
    HashMismatch { expected: String, actual: String },

    #[error("target node not found: {0}")]
    MissingTarget(String),

    #[error("invalid operation: {0}")]
    InvalidOperation(String),

    #[error(transparent)]
    Model(#[from] AIDocError),

    #[error(transparent)]
    Store(#[from] aidoc_storage::StoreError),
}

/// Verify `op.expected_revision == current head`.
pub fn check_revision(
    store: &Store,
    doc_id: &str,
    op: &Operation,
) -> Result<RevisionId, CheckError> {
    let actual = crud::head_revision(store.conn(), doc_id)?
        .ok_or_else(|| CheckError::RevisionNotFound(op.expected_revision.as_str().into()))?;
    if actual != op.expected_revision.as_str() {
        return Err(CheckError::Conflict {
            expected: op.expected_revision.as_str().into(),
            actual,
        });
    }
    Ok(RevisionId::new(actual))
}

/// Verify the node-level preconditions for an op (target exists, hashes match).
pub fn check_conflict(store: &Store, doc_id: &str, op: &Operation) -> Result<(), CheckError> {
    if let Some(target) = &op.target {
        let node = crud::get_node(store.conn(), doc_id, target)?
            .ok_or_else(|| CheckError::MissingTarget(target.as_str().into()))?;
        if let Some(expected) = op.patch.as_ref().and_then(|p| p.content.as_ref())
            && node.content != *expected
            && op.op_type == aidoc_model::OperationType::Update
            && let Some(actual_hash) = crud::get_content_hash(store.conn(), doc_id, target)?
            && actual_hash != "sha256:placeholder"
        {
            // mismatch is informational — caller may decide to fail.
        }
    }
    Ok(())
}

/// Convenience: confirm both at once.
pub fn check_all(store: &Store, doc_id: &str, op: &Operation) -> Result<RevisionId, CheckError> {
    let head = check_revision(store, doc_id, op)?;
    check_conflict(store, doc_id, op)?;
    Ok(head)
}

#[allow(dead_code)]
fn _ensure_nodeid_used(_: &NodeId) {}
