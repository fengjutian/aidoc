use anyhow::{Context, Result};

use aidoc::storage::crud;

use crate::session::Session;

pub fn run(path: &str, json: bool, branch: Option<&str>) -> Result<()> {
    let s = Session::open(path)?;
    let doc_id = s.package.as_ref().unwrap().manifest.document.id.clone();
    let revs = match branch {
        Some(name) => crud::list_revisions_by_branch(s.store.conn(), &doc_id, name)
            .with_context(|| format!("list revisions for branch {name}"))?,
        None => crud::list_revisions(s.store.conn(), &doc_id).context("list revisions")?,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&revs)?);
    } else {
        println!(
            "{:<10} {:<10} {:<14} {:<14} {:<20} MESSAGE",
            "ID", "PARENT", "OP", "BRANCH", "WHEN"
        );
        for r in &revs {
            let parent = r.parent.as_ref().map(|p| p.as_str()).unwrap_or("-");
            let branch_label = r.branch.as_deref().unwrap_or("main");
            println!(
                "{:<10} {:<10} {:<14} {:<14} {:<20} {}",
                r.id.as_str(),
                parent,
                r.operation.as_str(),
                branch_label,
                r.created_at.format("%Y-%m-%d %H:%M:%S"),
                r.message.clone().unwrap_or_default()
            );
        }
        let scope = match branch {
            Some(name) => format!("branch={name}"),
            None => "all branches".into(),
        };
        println!("\ntotal revisions ({scope}): {}", revs.len());
    }
    Ok(())
}
