//! `aidoc branch` — tag the current head with a named branch (spec §34).

use anyhow::{Context, Result};
use indexmap::IndexMap;

use aidoc::id::RevisionId;
use aidoc::storage::crud as storage_crud;
use aidoc::{Operation, OperationType, Patch, Provenance, apply_operation};

use crate::session::Session;

pub fn run(path: &str, name: &str, reason: Option<&str>) -> Result<()> {
    if name.trim().is_empty() || name == "main" {
        anyhow::bail!("branch name must be non-empty and not 'main' (got {name:?})");
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

    let mut attrs = IndexMap::new();
    attrs.insert("branch".into(), name.to_string());

    let op = Operation {
        id: aidoc::id::OpId::new(format!("OP-branch-{name}")),
        op_type: OperationType::Branch,
        target: None,
        expected_revision: RevisionId::new(head.clone()),
        expected_hash: None,
        target_revision: None,
        targets: vec![],
        actor: Provenance::human(Some("cli".into())),
        patch: Some(Patch {
                kind: None,
                position: None,
            content: None,
            title: None,
            semantic_type: None,
            attributes: attrs,
        }),
        reason: reason.map(String::from),
    };

    let outcome =
        apply_operation(&mut s.store, &doc_id, op).map_err(|e| anyhow::anyhow!("branch: {e}"))?;

    // Update manifest to track the branch head.
    if let Some(pkg) = s.package.as_mut() {
        pkg.manifest.set_revision(outcome.revision.as_str());
    }
    s.save().context("save package")?;

    println!(
        "branched {name} from {head} → {} (rev {})",
        outcome.revision.as_str(),
        outcome.revision.as_str()
    );
    Ok(())
}
