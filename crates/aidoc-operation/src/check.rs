//! Conflict detection / optimistic concurrency (spec §25-§26, §36).
//!
//! Two guards run before any mutation:
//!   * [`check_revision`] — §25 optimistic concurrency on `expected_revision`.
//!   * [`check_conflict`] — §26 content-hash guard plus the node / structure /
//!     relation preconditions enumerated by §36.
//!
//! Every failure is reported as a structured [`Conflict`] so the engine can
//! surface the machine-readable `{ "status": "conflict", ... }` shape instead of
//! silently overwriting a concurrent edit.

use aidoc_model::{
    Operation, OperationType,
    id::{AIDocError, NodeId, RevisionId},
};

use aidoc_storage::{Store, crud};

use crate::conflict::Conflict;

#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("revision not found: {0}")]
    RevisionNotFound(String),

    /// A §36 conflict class. Display always contains the word "conflict".
    #[error("{0}")]
    Conflict(#[from] Conflict),

    #[error("invalid operation: {0}")]
    InvalidOperation(String),

    #[error(transparent)]
    Model(#[from] AIDocError),

    #[error(transparent)]
    Store(#[from] aidoc_storage::StoreError),
}

/// Verify `op.expected_revision == current head` (§25).
pub fn check_revision(
    store: &Store,
    doc_id: &str,
    op: &Operation,
) -> Result<RevisionId, CheckError> {
    let actual = crud::head_revision(store.conn(), doc_id)?
        .ok_or_else(|| CheckError::RevisionNotFound(op.expected_revision.as_str().into()))?;
    if actual != op.expected_revision.as_str() {
        return Err(CheckError::Conflict(Conflict::revision(
            op.expected_revision.as_str(),
            actual,
        )));
    }
    Ok(RevisionId::new(actual))
}

/// Verify the node-level preconditions for an op (§26 content hash, §36
/// node / structure / relation conflicts).
pub fn check_conflict(store: &Store, doc_id: &str, op: &Operation) -> Result<(), CheckError> {
    // Create materialises a brand-new node and Branch only labels the current
    // head, so neither requires a pre-existing target.
    let requires_target = !matches!(op.op_type, OperationType::Create | OperationType::Branch);

    match &op.target {
        Some(target) => {
            let node = crud::get_node(store.conn(), doc_id, target)?;
            if requires_target && node.is_none() {
                return Err(CheckError::Conflict(Conflict::node(target.as_str())));
            }
            // §26 — optional content-hash guard. Only meaningful once the node
            // exists; a missing hash reads as empty and therefore conflicts.
            if let Some(expected) = &op.expected_hash {
                let actual = crud::get_content_hash(store.conn(), doc_id, target)?
                    .unwrap_or_default();
                if &actual != expected {
                    return Err(CheckError::Conflict(Conflict::content(
                        target.as_str(),
                        expected,
                        actual,
                    )));
                }
            }
        }
        None => {
            // Ops that mutate an existing node must carry a target.
            if matches!(
                op.op_type,
                OperationType::Update
                    | OperationType::Delete
                    | OperationType::Rename
                    | OperationType::Move
                    | OperationType::Replace
                    | OperationType::Split
                    | OperationType::Merge
            ) {
                return Err(CheckError::InvalidOperation(format!(
                    "{} requires a target node",
                    op.op_type.as_str()
                )));
            }
        }
    }

    match op.op_type {
        OperationType::Move => check_move_structure(store, doc_id, op)?,
        OperationType::Link | OperationType::Unlink => check_relation(store, doc_id, op)?,
        _ => {}
    }
    Ok(())
}

/// Convenience: confirm both at once.
pub fn check_all(store: &Store, doc_id: &str, op: &Operation) -> Result<RevisionId, CheckError> {
    let head = check_revision(store, doc_id, op)?;
    check_conflict(store, doc_id, op)?;
    Ok(head)
}

/// §36 STRUCTURE_CONFLICT — a move that names a new parent must point at an
/// existing node and must not create a cycle (parent == self or a descendant).
fn check_move_structure(store: &Store, doc_id: &str, op: &Operation) -> Result<(), CheckError> {
    let Some(target) = &op.target else {
        return Ok(());
    };
    let Some(new_parent) = op
        .patch
        .as_ref()
        .and_then(|p| p.attributes.get("parent"))
    else {
        // No reparent requested — nothing structural to validate.
        return Ok(());
    };
    if new_parent == target.as_str() {
        return Err(CheckError::Conflict(Conflict::structure(
            target.as_str(),
            format!("structure conflict: cannot move '{}' under itself", target.as_str()),
        )));
    }
    if new_parent.is_empty() {
        return Ok(());
    }
    let parent_id = NodeId::from_validated(new_parent.clone());
    if crud::get_node(store.conn(), doc_id, &parent_id)?.is_none() {
        return Err(CheckError::Conflict(Conflict::structure(
            target.as_str(),
            format!("structure conflict: new parent '{new_parent}' not found"),
        )));
    }
    // Walk up from the new parent; reaching the target means a cycle.
    let mut cur = Some(new_parent.clone());
    let mut guard = 0u32;
    while let Some(p) = cur {
        if p == target.as_str() {
            return Err(CheckError::Conflict(Conflict::structure(
                target.as_str(),
                format!("structure conflict: moving '{}' under '{new_parent}' creates a cycle", target.as_str()),
            )));
        }
        let node = crud::get_node(store.conn(), doc_id, &NodeId::from_validated(p))?;
        cur = node.and_then(|n| n.parent.map(|pp| pp.as_str().to_owned()));
        guard += 1;
        if guard > 10_000 {
            break;
        }
    }
    Ok(())
}

/// §36 RELATION_CONFLICT — link/unlink preconditions. `op.target` is the link
/// destination and `op.targets[0]` the source (mirrors the handlers).
fn check_relation(store: &Store, doc_id: &str, op: &Operation) -> Result<(), CheckError> {
    let Some(dst) = &op.target else {
        return Ok(());
    };
    let Some(src) = op.targets.first() else {
        // Missing source is reported by the handler as MissingSplitTargets.
        return Ok(());
    };
    if crud::get_node(store.conn(), doc_id, src)?.is_none() {
        return Err(CheckError::Conflict(Conflict::node(src.as_str())));
    }
    let rel_id = format!("rel-{}-{}", src.as_str(), dst.as_str());
    let exists = crud::list_relations(store.conn(), doc_id)?
        .iter()
        .any(|r| r.id == rel_id);
    match op.op_type {
        OperationType::Link if exists => Err(CheckError::Conflict(Conflict::relation(
            src.as_str(),
            format!("relation conflict: '{rel_id}' already exists"),
        ))),
        OperationType::Unlink if !exists => Err(CheckError::Conflict(Conflict::relation(
            src.as_str(),
            format!("relation conflict: '{rel_id}' does not exist"),
        ))),
        _ => Ok(()),
    }
}
