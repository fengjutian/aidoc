//! Merge a named branch into the checked-out main branch.

use crate::session::Session;
use aidoc::merge_branch;
use anyhow::{Context, Result};

pub fn run(path: &str, branch: &str, reason: Option<&str>) -> Result<()> {
    let mut session = Session::open(path)?;
    let doc_id = session
        .package
        .as_ref()
        .unwrap()
        .manifest
        .document
        .id
        .clone();
    let revision = merge_branch(
        &mut session.store,
        &doc_id,
        branch,
        reason.map(str::to_owned),
    )?;
    session.save().context("save package")?;
    println!("merged {branch} into main at {}", revision.as_str());
    Ok(())
}
