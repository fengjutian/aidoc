//! Branch checkout and conservative three-way node merge.

use std::collections::{BTreeMap, BTreeSet};

use aidoc_model::{Change, ChangeType, HashRef, Node, Operation, OperationType, Provenance, Revision};
use aidoc_model::id::{NodeId, OpId, RevisionId, sha256_hex};
use aidoc_storage::{Store, StoreError, crud};
use chrono::Utc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BranchError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("branch not found: {0}")]
    NotFound(String),
    #[error("branch operation requires main to be checked out")]
    NotOnMain,
    #[error("snapshot missing for revision {0}")]
    MissingSnapshot(String),
    #[error("no common ancestor for branch {0}")]
    NoCommonAncestor(String),
    #[error("merge conflict in nodes: {}", .0.join(", "))]
    Conflicts(Vec<String>),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub fn branch_head(store: &Store, doc_id: &str, name: &str) -> Result<String, BranchError> {
    let sql = if name == "main" {
        "SELECT r.id FROM revisions r LEFT JOIN revision_branches b ON b.doc_id=r.doc_id AND b.revision=r.id WHERE r.doc_id=?1 AND b.revision IS NULL ORDER BY CAST(SUBSTR(r.id,2) AS INTEGER) DESC LIMIT 1"
    } else {
        "SELECT r.id FROM revisions r JOIN revision_branches b ON b.doc_id=r.doc_id AND b.revision=r.id WHERE r.doc_id=?1 AND b.branch=?2 ORDER BY CAST(SUBSTR(r.id,2) AS INTEGER) DESC LIMIT 1"
    };
    use rusqlite::OptionalExtension;
    let found = if name == "main" {
        store.conn().query_row(sql, [doc_id], |r| r.get(0)).optional()?
    } else {
        store.conn().query_row(sql, rusqlite::params![doc_id, name], |r| r.get(0)).optional()?
    };
    found.ok_or_else(|| BranchError::NotFound(name.into()))
}

fn snapshot(store: &Store, doc_id: &str, rev: &str) -> Result<Vec<Node>, BranchError> {
    crud::load_snapshot(store.conn(), doc_id, rev)?
        .ok_or_else(|| BranchError::MissingSnapshot(rev.into()))
}

/// Switch the live node view and head to an existing branch tip. This does
/// not create a revision; the next edit descends from that tip.
pub fn checkout_branch(store: &mut Store, doc_id: &str, name: &str) -> Result<String, BranchError> {
    let tip = branch_head(store, doc_id, name)?;
    let nodes = snapshot(store, doc_id, &tip)?;
    store.tx(|tx| {
        tx.execute("DELETE FROM nodes WHERE doc_id=?1", [doc_id])?;
        for node in &nodes { crud::insert_node(tx, doc_id, node)?; }
        crud::advance_head(tx, doc_id, &tip)?;
        Ok::<_, BranchError>(())
    })?;
    Ok(tip)
}

fn ancestors(store: &Store, doc_id: &str, start: &str) -> Result<BTreeSet<String>, BranchError> {
    let mut seen = BTreeSet::new();
    let mut pending = vec![start.to_owned()];
    while let Some(rev) = pending.pop() {
        if seen.insert(rev.clone()) {
            pending.extend(crud::list_parents(store.conn(), doc_id, &rev)?);
        }
    }
    Ok(seen)
}

pub fn merge_branch(store: &mut Store, doc_id: &str, name: &str, reason: Option<String>) -> Result<RevisionId, BranchError> {
    if name == "main" || name.trim().is_empty() { return Err(BranchError::NotFound(name.into())); }
    if crud::head_branch(store.conn(), doc_id)?.is_some() { return Err(BranchError::NotOnMain); }
    let main = crud::head_revision(store.conn(), doc_id)?.ok_or_else(|| BranchError::NotFound("main".into()))?;
    let source = branch_head(store, doc_id, name)?;
    let main_ancestors = ancestors(store, doc_id, &main)?;
    let mut cursor = source.clone();
    let base = loop {
        if main_ancestors.contains(&cursor) { break cursor; }
        cursor = crud::list_parents(store.conn(), doc_id, &cursor)?.into_iter().next()
            .ok_or_else(|| BranchError::NoCommonAncestor(name.into()))?;
    };
    let base_nodes = snapshot(store, doc_id, &base)?;
    let main_nodes = snapshot(store, doc_id, &main)?;
    let source_nodes = snapshot(store, doc_id, &source)?;
    let map = |nodes: Vec<Node>| -> BTreeMap<String, Node> {
        nodes.into_iter().map(|n| (n.id.as_str().to_owned(), n)).collect()
    };
    let (base_map, main_map, source_map) = (map(base_nodes), map(main_nodes), map(source_nodes));
    let keys: BTreeSet<_> = base_map.keys().chain(main_map.keys()).chain(source_map.keys()).cloned().collect();
    let mut merged = BTreeMap::new();
    let mut conflicts = Vec::new();
    let mut changed = Vec::new();
    for id in keys {
        let (b, m, s) = (base_map.get(&id), main_map.get(&id), source_map.get(&id));
        let result = if m == s || s == b { m } else if m == b { s } else {
            conflicts.push(id.clone());
            continue;
        };
        if result != m { changed.push(id.clone()); }
        if let Some(node) = result { merged.insert(id, node.clone()); }
    }
    if !conflicts.is_empty() { return Err(BranchError::Conflicts(conflicts)); }
    // A merged tree must not strand children under a deleted parent.
    for node in merged.values() {
        if let Some(parent) = &node.parent {
            if !merged.contains_key(parent.as_str()) { return Err(BranchError::Conflicts(vec![node.id.as_str().into()])); }
        }
    }
    let seq = crud::max_revision_seq(store.conn(), doc_id)?;
    let revision = RevisionId::from_sequence(seq);
    let op_id = OpId::new(format!("OP-MERGE-{seq}"));
    let op = Operation {
        id: op_id.clone(), op_type: OperationType::Merge, target: None,
        expected_revision: RevisionId::new(main.clone()), expected_hash: None,
        target_revision: Some(RevisionId::new(source.clone())), targets: vec![],
        actor: Provenance::human(None), patch: None,
        reason: reason.or_else(|| Some(format!("merge {name} into main"))),
    };
    store.tx(|tx| {
        tx.execute("DELETE FROM nodes WHERE doc_id=?1", [doc_id])?;
        for node in merged.values() { crud::insert_node(tx, doc_id, node)?; }
        let now = Utc::now();
        crud::insert_operation(tx, doc_id, &op, now)?;
        crud::insert_revision(tx, doc_id, &Revision {
            id: revision.clone(), parent: Some(RevisionId::new(main.clone())),
            operation: op_id.clone(), created_at: now, message: op.reason.clone(), branch: None,
        }, true)?;
        tx.execute("INSERT INTO revision_parents(doc_id,revision,parent,seq) VALUES(?1,?2,?3,1)",
            rusqlite::params![doc_id, revision.as_str(), source])?;
        crud::advance_head(tx, doc_id, revision.as_str())?;
        for id in &changed {
            let before = main_map.get(id).map(|n| HashRef { hash: sha256_hex(n.content.as_bytes()) });
            let after = merged.get(id).map(|n| HashRef { hash: sha256_hex(n.content.as_bytes()) });
            crud::insert_change(tx, doc_id, &Change {
                id: format!("CH-{}-{id}", revision.as_str()), revision: revision.clone(),
                node: NodeId::from_validated(id.clone()), change_type: ChangeType::Merge,
                before, after, summary: Some(format!("merged from {name}")),
            })?;
        }
        let nodes: Vec<Node> = merged.values().cloned().collect();
        crud::save_snapshot(tx, doc_id, revision.as_str(), &nodes)?;
        Ok::<_, BranchError>(())
    })?;
    Ok(revision)
}
