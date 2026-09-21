//! Switch the live document to an existing branch tip.

use crate::session::Session;
use aidoc::checkout_branch;
use anyhow::{Context, Result};

pub fn run(path: &str, branch: &str) -> Result<()> {
    let mut session = Session::open(path)?;
    let doc_id = session
        .package
        .as_ref()
        .unwrap()
        .manifest
        .document
        .id
        .clone();
    let revision = checkout_branch(&mut session.store, &doc_id, branch)?;
    session.save().context("save package")?;
    println!("checked out {branch} at {revision}");
    Ok(())
}
