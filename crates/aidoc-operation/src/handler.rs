//! Pluggable operation handlers (spec §21 seam).
//!
//! The pre-refactor engine was one giant `match op.op_type { ... }` inside
//! `apply_operation`. Each arm interleaved three concerns: reading the intent
//! out of the [`Operation`], issuing `crud::*` writes against the open
//! transaction, and emitting the matching [`Change`] row.
//!
//! Here each arm becomes an [`OperationHandler`] that receives an
//! [`ApplyContext`] — a small bundle of `(tx, doc_id, new_rev, op)` plus
//! convenience wrappers for the common `crud` calls. `HandlerRegistry` maps
//! [`OperationType`] → handler, so a brand-new op is added by writing one
//! handler struct and registering it; the orchestration in `apply.rs` never
//! changes. Logic is moved verbatim from the old match to keep runtime
//! behaviour byte-identical.

use std::collections::HashMap;

use rusqlite::Transaction;

use aidoc_model::{
    Change, ChangeType, HashRef, Node, NodeId, NodeKind, Operation, OperationType, Patch, Relation,
    RelationKind,
    id::{RevisionId, sha256_hex},
};

use aidoc_storage::crud;

use crate::apply::ApplyError;

/// Transaction-scoped context handed to an [`OperationHandler`].
///
/// Wraps the open SQLite transaction plus the ids the handler needs, and
/// exposes the `crud` calls handlers actually use so each arm stays short.
pub struct ApplyContext<'a> {
    pub tx: &'a Transaction<'a>,
    pub doc_id: &'a str,
    pub new_rev: &'a RevisionId,
    pub op: &'a Operation,
}

impl ApplyContext<'_> {
    pub fn get_node(&self, id: &NodeId) -> Result<Option<Node>, ApplyError> {
        Ok(crud::get_node(self.tx, self.doc_id, id)?)
    }

    pub fn insert_node(&self, node: &Node) -> Result<(), ApplyError> {
        Ok(crud::insert_node(self.tx, self.doc_id, node)?)
    }

    pub fn update_node(&self, node: &Node) -> Result<String, ApplyError> {
        Ok(crud::update_node(self.tx, self.doc_id, node)?)
    }

    pub fn delete_node(&self, id: &NodeId) -> Result<(), ApplyError> {
        Ok(crud::delete_node(self.tx, self.doc_id, id)?)
    }

    pub fn get_content_hash(&self, id: &NodeId) -> Result<Option<String>, ApplyError> {
        Ok(crud::get_content_hash(self.tx, self.doc_id, id)?)
    }

    pub fn insert_relation(&self, rel: &Relation) -> Result<(), ApplyError> {
        Ok(crud::insert_relation(self.tx, self.doc_id, rel)?)
    }

    pub fn delete_relation(&self, rel_id: &str) -> Result<(), ApplyError> {
        Ok(crud::delete_relation(self.tx, self.doc_id, rel_id)?)
    }

    /// Emit a [`Change`] row against `self.new_rev`. Promoted from the old
    /// free function in `apply.rs`; `doc_id` / `rev` / `tx` now come from self.
    pub fn record_change(
        &self,
        node: &NodeId,
        change_type: ChangeType,
        before: Option<&String>,
        after: Option<&String>,
        summary: &str,
    ) -> Result<(), ApplyError> {
        let change_id = format!("CH-{}-{}", self.new_rev.as_str(), node.as_str());
        let ch = Change {
            id: change_id,
            revision: self.new_rev.clone(),
            node: node.clone(),
            change_type,
            before: before.map(|h| HashRef { hash: h.into() }),
            after: after.map(|h| HashRef { hash: h.into() }),
            summary: Some(summary.into()),
        };
        crud::insert_change(self.tx, self.doc_id, &ch)?;
        Ok(())
    }

    /// Convenience for the common "target must exist" lookup shared by
    /// Update / Rename / Move / Replace.
    fn require_target(&self, target: &NodeId) -> Result<Node, ApplyError> {
        self.get_node(target)?.ok_or_else(|| {
            ApplyError::Store(aidoc_storage::StoreError::Integrity(format!(
                "target not found: {}",
                target.as_str()
            )))
        })
    }
}

/// A handler for exactly one [`OperationType`].
pub trait OperationHandler {
    fn op_type(&self) -> OperationType;
    fn apply(&self, ctx: &mut ApplyContext) -> Result<(), ApplyError>;
}

// ---------- Built-in handlers (spec §21) ----------

pub struct CreateHandler;

impl OperationHandler for CreateHandler {
    fn op_type(&self) -> OperationType {
        OperationType::Create
    }
    fn apply(&self, ctx: &mut ApplyContext) -> Result<(), ApplyError> {
        let target = ctx
            .op
            .target
            .clone()
            .ok_or(ApplyError::MissingTarget("create"))?;
        let patch = ctx.op.patch.clone().ok_or(ApplyError::MissingPatch)?;
        let node = materialize_create(&target, &patch)?;
        let after_hash = sha256_hex(node.content.as_bytes());
        ctx.insert_node(&node)?;
        ctx.record_change(
            &target,
            ChangeType::Create,
            None,
            Some(&after_hash),
            "create node",
        )?;
        Ok(())
    }
}

pub struct UpdateHandler;

impl OperationHandler for UpdateHandler {
    fn op_type(&self) -> OperationType {
        OperationType::Update
    }
    fn apply(&self, ctx: &mut ApplyContext) -> Result<(), ApplyError> {
        let target = ctx
            .op
            .target
            .clone()
            .ok_or(ApplyError::MissingTarget("update"))?;
        let patch = ctx.op.patch.clone().ok_or(ApplyError::MissingPatch)?;
        let mut node = ctx.require_target(&target)?;
        let before_hash = sha256_hex(node.content.as_bytes());
        apply_patch_to_node(&mut node, &patch);
        let after_hash = ctx.update_node(&node)?;
        ctx.record_change(
            &target,
            ChangeType::ContentUpdate,
            Some(&before_hash),
            Some(&after_hash),
            "update node",
        )?;
        Ok(())
    }
}

pub struct DeleteHandler;

impl OperationHandler for DeleteHandler {
    fn op_type(&self) -> OperationType {
        OperationType::Delete
    }
    fn apply(&self, ctx: &mut ApplyContext) -> Result<(), ApplyError> {
        let target = ctx
            .op
            .target
            .clone()
            .ok_or(ApplyError::MissingTarget("delete"))?;
        let before_node = ctx.get_node(&target)?;
        let before_hash = before_node
            .as_ref()
            .map(|n| sha256_hex(n.content.as_bytes()))
            .unwrap_or_default();
        ctx.delete_node(&target)?;
        ctx.record_change(
            &target,
            ChangeType::Delete,
            Some(&before_hash),
            None,
            "delete node",
        )?;
        Ok(())
    }
}

pub struct RenameHandler;

impl OperationHandler for RenameHandler {
    fn op_type(&self) -> OperationType {
        OperationType::Rename
    }
    fn apply(&self, ctx: &mut ApplyContext) -> Result<(), ApplyError> {
        let target = ctx
            .op
            .target
            .clone()
            .ok_or(ApplyError::MissingTarget("rename"))?;
        let patch = ctx.op.patch.clone().ok_or(ApplyError::MissingPatch)?;
        let mut node = ctx.require_target(&target)?;
        let before_hash = sha256_hex(node.content.as_bytes());
        apply_patch_to_node(&mut node, &patch);
        let after_hash = ctx.update_node(&node)?;
        ctx.record_change(
            &target,
            ChangeType::Rename,
            Some(&before_hash),
            Some(&after_hash),
            "rename node",
        )?;
        Ok(())
    }
}

pub struct MoveHandler;

impl OperationHandler for MoveHandler {
    fn op_type(&self) -> OperationType {
        OperationType::Move
    }
    fn apply(&self, ctx: &mut ApplyContext) -> Result<(), ApplyError> {
        let target = ctx
            .op
            .target
            .clone()
            .ok_or(ApplyError::MissingTarget("move"))?;
        let patch = ctx.op.patch.clone().ok_or(ApplyError::MissingPatch)?;
        let mut node = ctx.require_target(&target)?;
        let before_hash = sha256_hex(node.content.as_bytes());
        apply_patch_to_node(&mut node, &patch);
        let after_hash = ctx.update_node(&node)?;
        ctx.record_change(
            &target,
            ChangeType::Move,
            Some(&before_hash),
            Some(&after_hash),
            "move node",
        )?;
        Ok(())
    }
}

pub struct ReplaceHandler;

impl OperationHandler for ReplaceHandler {
    fn op_type(&self) -> OperationType {
        OperationType::Replace
    }
    fn apply(&self, ctx: &mut ApplyContext) -> Result<(), ApplyError> {
        let target = ctx
            .op
            .target
            .clone()
            .ok_or(ApplyError::MissingTarget("replace"))?;
        let patch = ctx.op.patch.clone().ok_or(ApplyError::MissingPatch)?;
        let mut node = ctx.require_target(&target)?;
        let before_hash = sha256_hex(node.content.as_bytes());
        apply_patch_to_node(&mut node, &patch);
        let after_hash = ctx.update_node(&node)?;
        ctx.record_change(
            &target,
            ChangeType::ContentUpdate,
            Some(&before_hash),
            Some(&after_hash),
            "replace node",
        )?;
        Ok(())
    }
}

pub struct SplitHandler;

impl OperationHandler for SplitHandler {
    fn op_type(&self) -> OperationType {
        OperationType::Split
    }
    fn apply(&self, ctx: &mut ApplyContext) -> Result<(), ApplyError> {
        let source = ctx
            .op
            .target
            .clone()
            .ok_or(ApplyError::MissingTarget("split"))?;
        if ctx.op.targets.is_empty() {
            return Err(ApplyError::MissingSplitTargets);
        }
        let source_node = ctx.get_node(&source)?.ok_or_else(|| {
            ApplyError::Store(aidoc_storage::StoreError::Integrity(format!(
                "split source not found: {}",
                source.as_str()
            )))
        })?;
        let before_hash = sha256_hex(source_node.content.as_bytes());
        // Replace source with first target; create the rest.
        let targets = ctx.op.targets.clone();
        for (i, target_id) in targets.iter().enumerate() {
            let new_node = Node {
                id: target_id.clone(),
                kind: source_node.kind,
                parent: source_node.parent.clone(),
                position: source_node.position + i as u32,
                semantic_type: source_node.semantic_type.clone(),
                content: if i == 0 {
                    source_node.content.clone()
                } else {
                    String::new()
                },
                attributes: source_node.attributes.clone(),
            };
            ctx.insert_node(&new_node)?;
        }
        ctx.delete_node(&source)?;
        // Keep the source subtree attached to the first replacement node.
        for mut child in crud::list_nodes(ctx.tx, ctx.doc_id)? {
            if child.parent.as_ref() == Some(&source) {
                child.parent = Some(targets[0].clone());
                ctx.update_node(&child)?;
            }
        }
        ctx.record_change(
            &source,
            ChangeType::Split,
            Some(&before_hash),
            None,
            &format!("split into {} targets", targets.len()),
        )?;
        Ok(())
    }
}

pub struct MergeHandler;

impl OperationHandler for MergeHandler {
    fn op_type(&self) -> OperationType {
        OperationType::Merge
    }
    fn apply(&self, ctx: &mut ApplyContext) -> Result<(), ApplyError> {
        let target = ctx
            .op
            .target
            .clone()
            .ok_or(ApplyError::MissingTarget("merge"))?;
        if ctx.op.targets.is_empty() {
            return Err(ApplyError::MissingSplitTargets);
        }
        // Merge: keep target, delete sources.
        let target_node = ctx.get_node(&target)?.ok_or_else(|| {
            ApplyError::Store(aidoc_storage::StoreError::Integrity(format!(
                "merge target not found: {}",
                target.as_str()
            )))
        })?;
        let before_hash = sha256_hex(target_node.content.as_bytes());
        let sources = ctx.op.targets.clone();
        for src in &sources {
            ctx.delete_node(src)?;
        }
        let after_hash = ctx
            .get_content_hash(&target)?
            .unwrap_or(before_hash.clone());
        ctx.record_change(
            &target,
            ChangeType::Merge,
            Some(&before_hash),
            Some(&after_hash),
            &format!("merge {} sources", sources.len()),
        )?;
        Ok(())
    }
}

pub struct LinkHandler;

impl OperationHandler for LinkHandler {
    fn op_type(&self) -> OperationType {
        OperationType::Link
    }
    fn apply(&self, ctx: &mut ApplyContext) -> Result<(), ApplyError> {
        // Targets[0] is link source, target is link target.
        let link_src = ctx
            .op
            .targets
            .first()
            .cloned()
            .ok_or(ApplyError::MissingSplitTargets)?;
        let link_dst = ctx
            .op
            .target
            .clone()
            .ok_or(ApplyError::MissingTarget("link"))?;
        let mut attributes = ctx
            .op
            .patch
            .as_ref()
            .map(|patch| patch.attributes.clone())
            .unwrap_or_default();
        let relation_kind = attributes
            .shift_remove("relation_kind")
            .unwrap_or_else(|| "references".into());
        let kind = RelationKind::parse(&relation_kind);
        let rel = Relation {
            id: format!("rel-{}-{}", link_src.as_str(), link_dst.as_str()),
            source: link_src.clone(),
            target: link_dst.clone(),
            kind,
            custom_kind: (kind == RelationKind::Custom).then_some(relation_kind),
            attributes,
        };
        ctx.insert_relation(&rel)?;
        ctx.record_change(
            &link_src,
            ChangeType::RelationAdd,
            None,
            None,
            "link relation added",
        )?;
        Ok(())
    }
}

pub struct UnlinkHandler;

impl OperationHandler for UnlinkHandler {
    fn op_type(&self) -> OperationType {
        OperationType::Unlink
    }
    fn apply(&self, ctx: &mut ApplyContext) -> Result<(), ApplyError> {
        let link_src = ctx
            .op
            .targets
            .first()
            .cloned()
            .ok_or(ApplyError::MissingSplitTargets)?;
        let link_dst = ctx
            .op
            .target
            .clone()
            .ok_or(ApplyError::MissingTarget("unlink"))?;
        let rel_id = format!("rel-{}-{}", link_src.as_str(), link_dst.as_str());
        ctx.delete_relation(&rel_id)?;
        ctx.record_change(
            &link_src,
            ChangeType::RelationRemove,
            None,
            None,
            "link relation removed",
        )?;
        Ok(())
    }
}

pub struct BranchHandler;

impl OperationHandler for BranchHandler {
    fn op_type(&self) -> OperationType {
        OperationType::Branch
    }
    fn apply(&self, ctx: &mut ApplyContext) -> Result<(), ApplyError> {
        // v0.1 Branch (spec §34): record a named-branch revision that
        // shares the current head as its parent. The branch name comes
        // from `patch.attributes["branch"]` or, as a fallback, the
        // `reason` field. Nothing else mutates — branching is just
        // labelling the current state. Merge (spec §35) is the same
        // shape with two parents; v0.1 stores up to N parents in the
        // `revision_parents` side-table.
        let branch_name = ctx
            .op
            .patch
            .as_ref()
            .and_then(|p| p.attributes.get("branch").cloned())
            .or_else(|| ctx.op.reason.clone())
            .ok_or_else(|| {
                ApplyError::Invalid(
                    "branch op needs a branch name (patch.attributes[\"branch\"] or reason)".into(),
                )
            })?;
        if branch_name.trim().is_empty() || branch_name == "main" {
            return Err(ApplyError::Invalid(format!(
                "branch name must be non-empty and not 'main' (got {branch_name:?})"
            )));
        }
        // Branch only attaches a label; nothing to mutate in nodes.
        Ok(())
    }
}

// ---------- Registry ----------

/// Maps [`OperationType`] → handler. `HandlerRegistry::default()` registers
/// every built-in op; [`HandlerRegistry::register`] adds (or overrides) one.
pub struct HandlerRegistry {
    handlers: HashMap<OperationType, Box<dyn OperationHandler>>,
}

impl HandlerRegistry {
    /// Empty registry — register exactly the handlers you need.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Add a handler, keyed by the [`OperationType`] it reports. Registering a
    /// second handler for the same type replaces the first (extension point).
    pub fn register(&mut self, handler: Box<dyn OperationHandler>) {
        self.handlers.insert(handler.op_type(), handler);
    }

    /// Look up the handler for `op_type`, if one is registered.
    pub fn get(&self, op_type: OperationType) -> Option<&dyn OperationHandler> {
        self.handlers.get(&op_type).map(|h| h.as_ref())
    }
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        let mut r = Self::new();
        r.register(Box::new(CreateHandler));
        r.register(Box::new(UpdateHandler));
        r.register(Box::new(DeleteHandler));
        r.register(Box::new(RenameHandler));
        r.register(Box::new(MoveHandler));
        r.register(Box::new(ReplaceHandler));
        r.register(Box::new(SplitHandler));
        r.register(Box::new(MergeHandler));
        r.register(Box::new(LinkHandler));
        r.register(Box::new(UnlinkHandler));
        r.register(Box::new(BranchHandler));
        // NB: Revert is intentionally NOT registered — it is orchestrated by
        // `aidoc-history`, and dispatching it here yields `UnknownOperation`
        // exactly as the pre-refactor match arm did.
        r
    }
}

// ---------- Shared patch helpers (moved verbatim from apply.rs) ----------

fn materialize_create(id: &NodeId, patch: &Patch) -> Result<Node, ApplyError> {
    let mut node = Node::new(id.clone(), NodeKind::Generic);
    apply_patch_to_node(&mut node, patch);
    Ok(node)
}

fn apply_patch_to_node(node: &mut Node, patch: &Patch) {
    if let Some(content) = &patch.content {
        node.content = content.clone();
    }
    if let Some(k) = patch.kind {
        node.kind = k;
    }
    if let Some(p) = patch.position {
        node.position = p;
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
        // `"parent"` is the reparent convention shared with
        // `check::check_move_structure`: it retargets the structural parent
        // instead of landing in the free-form attribute bag. An empty value
        // detaches the node (moves it up to the document root).
        if k == "parent" {
            node.parent = if v.is_empty() {
                None
            } else {
                Some(NodeId::from_validated(v.clone()))
            };
        } else if v.is_empty() {
            node.attributes.remove(k);
        } else {
            node.attributes.insert(k.clone(), v.clone());
        }
    }
}
