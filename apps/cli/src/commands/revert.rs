use anyhow::{Result, anyhow};

use aidoc::history;
use aidoc::id::RevisionId;

use crate::session::Session;

pub fn run(path: &str, target: &str, reason: Option<&str>) -> Result<()> {
    let mut s = Session::open(path)?;
    let doc_id = s.package.as_ref().unwrap().manifest.document.id.clone();
    let target = RevisionId::new(target);
    let outcome = history::revert_to(
        &mut s.store,
        &doc_id,
        target.clone(),
        reason.map(|s| s.to_owned()),
    )
    .map_err(|e| anyhow!("revert failed: {e}"))?;

    if let Some(pkg) = s.package.as_mut() {
        pkg.manifest.set_revision(outcome.new_revision.as_str());
    }
    s.save()?;

    println!(
        "reverted to {} as new revision {} ({} nodes touched)",
        outcome.target_revision.as_str(),
        outcome.new_revision.as_str(),
        outcome.nodes_touched.len()
    );
    Ok(())
}
