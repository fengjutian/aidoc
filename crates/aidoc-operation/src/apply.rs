//! Apply an Operation to the store, atomically producing a new Revision.

use chrono::Utc;
use thiserror::Error;

use aidoc_model::{
    Change, ChangeType, HashRef, Operation, OperationType, Patch, Provenance, Relation,
    RelationKind,
    id::{AIDocError, OpId, RevisionId, sha256_hex},
};

use aidoc_storage::{Store, crud};

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

/// Apply a single forward op under one transaction.
pub fn apply_operation(
    store: &mut Store,
    doc_id: &str,
    op: Operation,
) -> Result<ApplyOutcome, ApplyError> {
    // 1. Optimistic concurrency check (read-only, no tx needed).
    crate::check::check_revision(store, doc_id, &op)
        .map_err(|e| ApplyError::Store(aidoc_storage::StoreError::Integrity(e.to_string())))?;

    // 2. Materialize the new revision id BEFORE we enter the tx.
    let next_seq = crud::max_revision_seq(store.conn(), doc_id).map_err(ApplyError::Store)?;
    let new_rev = RevisionId::from_sequence(next_seq);
    let op_id = op.id.clone();
    let actor_json = serde_json::to_string(&op.actor)
        .map_err(|e| ApplyError::Store(aidoc_storage::StoreError::Integrity(e.to_string())))?;
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
    let patch_json = op
        .patch
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| ApplyError::Store(aidoc_storage::StoreError::Integrity(e.to_string())))?;

    // 3. Apply inside a single SQLite transaction.
    store.tx(|tx| {
        // 3a. Advance head = 0 first; the new revision becomes head.
        // (advance_head is called at end; here we just ensure old head is cleared.)
        // The function does both, so we call at end.

        let parent_rev = op.expected_revision.clone();

        match op.op_type {
            OperationType::Create => {
                let target = op
                    .target
                    .clone()
                    .ok_or(ApplyError::MissingTarget("create"))?;
                let patch = op.patch.clone().ok_or(ApplyError::MissingPatch)?;
                let node = materialize_create(&target, &patch)?;
                let after_hash = sha256_hex(node.content.as_bytes());
                crud::insert_node(tx, doc_id, &node)?;
                record_change(tx, doc_id, &new_rev, &target, ChangeType::Create, None, Some(&after_hash), "create node")?;
            }

            OperationType::Update => {
                let target = op
                    .target
                    .clone()
                    .ok_or(ApplyError::MissingTarget("update"))?;
                let patch = op.patch.clone().ok_or(ApplyError::MissingPatch)?;
                let mut node = crud::get_node(tx, doc_id, &target)?
                    .ok_or_else(|| ApplyError::Store(aidoc_storage::StoreError::Integrity(
                        format!("target not found: {}", target.as_str()),
                    )))?;
                let before_hash = sha256_hex(node.content.as_bytes());
                apply_patch_to_node(&mut node, &patch);
                let after_hash = crud::update_node(tx, doc_id, &node)?;
                record_change(
                    tx,
                    doc_id,
                    &new_rev,
                    &target,
                    ChangeType::ContentUpdate,
                    Some(&before_hash),
                    Some(&after_hash),
                    "update node",
                )?;
            }

            OperationType::Delete => {
                let target = op
                    .target
                    .clone()
                    .ok_or(ApplyError::MissingTarget("delete"))?;
                let before_node = crud::get_node(tx, doc_id, &target)?;
                let before_hash = before_node
                    .as_ref()
                    .map(|n| sha256_hex(n.content.as_bytes()))
                    .unwrap_or_default();
                crud::delete_node(tx, doc_id, &target)?;
                record_change(
                    tx,
                    doc_id,
                    &new_rev,
                    &target,
                    ChangeType::Delete,
                    Some(&before_hash),
                    None,
                    "delete node",
                )?;
            }

            OperationType::Rename => {
                let target = op
                    .target
                    .clone()
                    .ok_or(ApplyError::MissingTarget("rename"))?;
                let patch = op.patch.clone().ok_or(ApplyError::MissingPatch)?;
                let mut node = crud::get_node(tx, doc_id, &target)?
                    .ok_or_else(|| ApplyError::Store(aidoc_storage::StoreError::Integrity(
                        format!("target not found: {}", target.as_str()),
                    )))?;
                let before_hash = sha256_hex(node.content.as_bytes());
                apply_patch_to_node(&mut node, &patch);
                let after_hash = crud::update_node(tx, doc_id, &node)?;
                record_change(
                    tx,
                    doc_id,
                    &new_rev,
                    &target,
                    ChangeType::Rename,
                    Some(&before_hash),
                    Some(&after_hash),
                    "rename node",
                )?;
            }

            OperationType::Move => {
                let target = op
                    .target
                    .clone()
                    .ok_or(ApplyError::MissingTarget("move"))?;
                let patch = op.patch.clone().ok_or(ApplyError::MissingPatch)?;
                let mut node = crud::get_node(tx, doc_id, &target)?
                    .ok_or_else(|| ApplyError::Store(aidoc_storage::StoreError::Integrity(
                        format!("target not found: {}", target.as_str()),
                    )))?;
                let before_hash = sha256_hex(node.content.as_bytes());
                apply_patch_to_node(&mut node, &patch);
                let after_hash = crud::update_node(tx, doc_id, &node)?;
                record_change(
                    tx,
                    doc_id,
                    &new_rev,
                    &target,
                    ChangeType::Move,
                    Some(&before_hash),
                    Some(&after_hash),
                    "move node",
                )?;
            }

            OperationType::Replace => {
                let target = op
                    .target
                    .clone()
                    .ok_or(ApplyError::MissingTarget("replace"))?;
                let patch = op.patch.clone().ok_or(ApplyError::MissingPatch)?;
                let mut node = crud::get_node(tx, doc_id, &target)?
                    .ok_or_else(|| ApplyError::Store(aidoc_storage::StoreError::Integrity(
                        format!("target not found: {}", target.as_str()),
                    )))?;
                let before_hash = sha256_hex(node.content.as_bytes());
                apply_patch_to_node(&mut node, &patch);
                let after_hash = crud::update_node(tx, doc_id, &node)?;
                record_change(
                    tx,
                    doc_id,
                    &new_rev,
                    &target,
                    ChangeType::ContentUpdate,
                    Some(&before_hash),
                    Some(&after_hash),
                    "replace node",
                )?;
            }

            OperationType::Split => {
                let source = op
                    .target
                    .clone()
                    .ok_or(ApplyError::MissingTarget("split"))?;
                if op.targets.is_empty() {
                    return Err(ApplyError::MissingSplitTargets);
                }
                let source_node = crud::get_node(tx, doc_id, &source)?
                    .ok_or_else(|| ApplyError::Store(aidoc_storage::StoreError::Integrity(
                        format!("split source not found: {}", source.as_str()),
                    )))?;
                let before_hash = sha256_hex(source_node.content.as_bytes());
                // Replace source with first target; create the rest.
                for (i, target_id) in op.targets.iter().enumerate() {
                    let new_node = aidoc_model::Node {
                        id: target_id.clone(),
                        kind: source_node.kind,
                        parent: source_node.parent.clone(),
                        position: source_node.position + i as u32,
                        semantic_type: source_node.semantic_type.clone(),
                        content: if i == 0 { source_node.content.clone() } else { String::new() },
                        attributes: source_node.attributes.clone(),
                    };
                    crud::insert_node(tx, doc_id, &new_node)?;
                }
                crud::delete_node(tx, doc_id, &source)?;
                record_change(
                    tx,
                    doc_id,
                    &new_rev,
                    &source,
                    ChangeType::Split,
                    Some(&before_hash),
                    None,
                    &format!("split into {} targets", op.targets.len()),
                )?;
            }

            OperationType::Merge => {
                let target = op
                    .target
                    .clone()
                    .ok_or(ApplyError::MissingTarget("merge"))?;
                if op.targets.is_empty() {
                    return Err(ApplyError::MissingSplitTargets);
                }
                // Merge: keep target, delete sources.
                let target_node = crud::get_node(tx, doc_id, &target)?
                    .ok_or_else(|| ApplyError::Store(aidoc_storage::StoreError::Integrity(
                        format!("merge target not found: {}", target.as_str()),
                    )))?;
                let before_hash = sha256_hex(target_node.content.as_bytes());
                for src in &op.targets {
                    crud::delete_node(tx, doc_id, src)?;
                }
                let after_hash = crud::get_content_hash(tx, doc_id, &target)?
                    .unwrap_or(before_hash.clone());
                record_change(
                    tx,
                    doc_id,
                    &new_rev,
                    &target,
                    ChangeType::Merge,
                    Some(&before_hash),
                    Some(&after_hash),
                    &format!("merge {} sources", op.targets.len()),
                )?;
            }

            OperationType::Link => {
                // Targets[0] is link source, target is link target.
                let link_src = op
                    .targets
                    .first()
                    .cloned()
                    .ok_or(ApplyError::MissingSplitTargets)?;
                let link_dst = op
                    .target
                    .clone()
                    .ok_or(ApplyError::MissingTarget("link"))?;
                let rel = Relation {
                    id: format!("rel-{}-{}", link_src.as_str(), link_dst.as_str()),
                    source: link_src.clone(),
                    target: link_dst.clone(),
                    kind: RelationKind::References,
                    custom_kind: None,
                };
                crud::insert_relation(tx, doc_id, &rel)?;
                record_change(
                    tx,
                    doc_id,
                    &new_rev,
                    &link_src,
                    ChangeType::RelationAdd,
                    None,
                    None,
                    "link relation added",
                )?;
            }

            OperationType::Unlink => {
                let link_src = op
                    .targets
                    .first()
                    .cloned()
                    .ok_or(ApplyError::MissingSplitTargets)?;
                let link_dst = op
                    .target
                    .clone()
                    .ok_or(ApplyError::MissingTarget("unlink"))?;
                let rel_id = format!("rel-{}-{}", link_src.as_str(), link_dst.as_str());
                crud::delete_relation(tx, doc_id, &rel_id)?;
                record_change(
                    tx,
                    doc_id,
                    &new_rev,
                    &link_src,
                    ChangeType::RelationRemove,
                    None,
                    None,
                    "link relation removed",
                )?;
            }

            OperationType::Revert => {
                // Revert is implemented in aidoc-history, not here.
                return Err(ApplyError::UnknownOperation("revert".into()));
            }
            OperationType::Branch => {
                // v0.1 Branch (spec §34): record a named-branch revision that
                // shares the current head as its parent. The branch name comes
                // from `patch.attributes["branch"]` or, as a fallback, the
                // `reason` field. Nothing else mutates — branching is just
                // labelling the current state. Merge (spec §35) is the same
                // shape with two parents; v0.1 stores up to N parents in the
                // `revision_parents` side-table.
                let branch_name = op
                    .patch
                    .as_ref()
                    .and_then(|p| p.attributes.get("branch").cloned())
                    .or_else(|| op.reason.clone())
                    .ok_or_else(|| {
                        ApplyError::Invalid(
                            "branch op needs a branch name (patch.attributes[\"branch\"] or reason)"
                                .into(),
                        )
                    })?;
                if branch_name.trim().is_empty() || branch_name == "main" {
                    return Err(ApplyError::Invalid(format!(
                        "branch name must be non-empty and not 'main' (got {branch_name:?})"
                    )));
                }
                // Branch only attaches a label; nothing to mutate in nodes.
            }
        }

        // 4. Write Operation + Revision + advance head.
        let created_at = Utc::now();
        tx.execute(
            r#"INSERT INTO operations(doc_id, id, op_type, target, expected_revision, target_revision, actor_json, patch_json, reason, created_at)
               VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
            rusqlite::params![
                doc_id,
                op_id.as_str(),
                op.op_type.as_str(),
                op.target.as_ref().map(|n| n.as_str().to_owned()),
                op.expected_revision.as_str(),
                op.target_revision.as_ref().map(|r| r.as_str().to_owned()),
                actor_json,
                patch_json,
                op.reason.as_deref(),
                created_at.to_rfc3339()
            ],
        )?;

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
        crud::save_snapshot(tx, doc_id, new_rev.as_str(), &live)
            .map_err(ApplyError::Store)?;

        // Keep the borrow checker happy.
        let _ = parent_rev;

        Ok(())
    })?;

    Ok(ApplyOutcome {
        revision: new_rev,
        op_id,
    })
}

fn materialize_create(
    id: &aidoc_model::id::NodeId,
    patch: &Patch,
) -> Result<aidoc_model::Node, ApplyError> {
    let mut node = aidoc_model::Node::new(id.clone(), aidoc_model::NodeKind::Generic);
    apply_patch_to_node(&mut node, patch);
    Ok(node)
}

fn apply_patch_to_node(node: &mut aidoc_model::Node, patch: &Patch) {
    if let Some(content) = &patch.content {
        node.content = content.clone();
    }
    if let Some(title) = &patch.title {
        // For semantic nodes, title is the first text child — we just stash it
        // in content if content is empty.
        if node.content.is_empty() {
            node.content = title.clone();
        }
        // For heading/section nodes we keep title in semantic_type as a hint.
        node.semantic_type = Some("titled".into());
    }
    if let Some(st) = &patch.semantic_type {
        node.semantic_type = Some(st.clone());
    }
    for (k, v) in &patch.attributes {
        node.attributes.insert(k.clone(), v.clone());
    }
}

#[allow(clippy::too_many_arguments)]
fn record_change(
    tx: &rusqlite::Transaction<'_>,
    doc_id: &str,
    rev: &RevisionId,
    node: &aidoc_model::id::NodeId,
    change_type: ChangeType,
    before: Option<&String>,
    after: Option<&String>,
    summary: &str,
) -> Result<(), ApplyError> {
    let change_id = format!("CH-{}-{}", rev.as_str(), node.as_str());
    let ch = Change {
        id: change_id,
        revision: rev.clone(),
        node: node.clone(),
        change_type,
        before: before.map(|h| HashRef { hash: h.into() }),
        after: after.map(|h| HashRef { hash: h.into() }),
        summary: Some(summary.into()),
    };
    crud::insert_change(tx, doc_id, &ch)?;
    Ok(())
}

// silence unused Provenance import when we don't reference it directly
#[allow(dead_code)]
fn _force_use(_: Provenance) {}

// `crud::max_revision_seq` owns the R### math now; see aidoc-storage.
