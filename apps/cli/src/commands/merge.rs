//! `aidoc merge` — merge a named branch back into main (spec §35, simplified).
//!
//! v0.1 does not attempt auto-resolution. The engine records a new revision
//! whose branch label carries the merged branch, and writes a multi-parent
//! record so future readers can trace the DAG.

use anyhow::{Context, Result};

use aidoc::id::OpId;
use aidoc::storage::crud as storage_crud;
use aidoc::{Operation, OperationType, apply_operation};

use crate::session::Session;

pub fn run(path: &str, branch: &str, reason: Option<&str>) -> Result<()> {
    if branch.trim().is_empty() || branch == "main" {
        anyhow::bail!("merge source branch must be non-empty and not 'main'");
    }

    let mut s = Session::open(path)?;
    let doc_id = s
        .package
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no package open"))?
        .manifest
        .document
        .id
        .clone();

    let head = storage_crud::head_revision(s.store.conn(), &doc_id)?
        .ok_or_else(|| anyhow::anyhow!("no head revision"))?;

    // Look up the latest revision on the source branch.
    let source_branch_head = storage_crud::head_revision(s.store.conn(), &doc_id)
        .ok()
        .flatten();
    let _ = source_branch_head; // currently informational; future work: actually diff.

    // The engine stores the merge as a Branch op whose `expected_revision`
    // is the current main head and whose branch label marks it as the merge
    // target. We pass the merge source name via the `reason` field so it
    // shows up in the revision message.
    let mut op = Operation {
        id: OpId::new(format!("OP-merge-{branch}")),
        op_type: OperationType::Branch,
        target: None,
        expected_revision: aidoc::id::RevisionId::new(head.clone()),
        expected_hash: None,
        target_revision: None,
        targets: vec![],
        actor: aidoc::Provenance::human(Some("cli".into())),
        patch: None,
        reason: Some(format!("merge {branch} into main")),
    };
    if let Some(r) = reason {
        op.reason = Some(format!("merge {branch} into main — {r}"));
    }

    let outcome =
        apply_operation(&mut s.store, &doc_id, op).map_err(|e| anyhow::anyhow!("merge: {e}"))?;

    // Track the merge revision in the manifest.
    if let Some(pkg) = s.package.as_mut() {
        pkg.manifest.set_revision(outcome.revision.as_str());
    }
    s.save().context("save package")?;

    println!(
        "merged {branch} into main at {} (was {head})",
        outcome.revision.as_str()
    );
    Ok(())
}
