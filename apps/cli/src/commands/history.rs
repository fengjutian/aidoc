use anyhow::{Context, Result};

use aidoc::storage::crud;

use crate::session::Session;

pub fn run(path: &str, json: bool) -> Result<()> {
    let s = Session::open(path)?;
    let doc_id = s.package.as_ref().unwrap().manifest.document.id.clone();
    let revs = crud::list_revisions(s.store.conn(), &doc_id).context("list revisions")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&revs)?);
    } else {
        println!(
            "{:<10} {:<10} {:<14} {:<20} MESSAGE",
            "ID", "PARENT", "OP", "WHEN"
        );
        for r in &revs {
            let parent = r.parent.as_ref().map(|p| p.as_str()).unwrap_or("-");
            println!(
                "{:<10} {:<10} {:<14} {:<20} {}",
                r.id.as_str(),
                parent,
                r.operation.as_str(),
                r.created_at.format("%Y-%m-%d %H:%M:%S"),
                r.message.clone().unwrap_or_default()
            );
        }
        println!("\ntotal revisions: {}", revs.len());
    }
    Ok(())
}
