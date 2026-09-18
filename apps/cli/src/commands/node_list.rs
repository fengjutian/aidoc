use anyhow::{Context, Result};

use aidoc::storage::crud;

use crate::session::Session;

pub fn run(path: &str, json: bool) -> Result<()> {
    let s = Session::open(path)?;
    let doc_id = s.package.as_ref().unwrap().manifest.document.id.clone();
    let nodes = crud::list_nodes(s.store.conn(), &doc_id).context("list nodes")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&nodes)?);
    } else {
        println!(
            "{:<24} {:<14} {:<24} {:>4}  CONTENT",
            "ID", "KIND", "PARENT", "POS"
        );
        for n in &nodes {
            let snippet: String = n.content.chars().take(60).collect();
            let parent = n.parent.as_ref().map(|p| p.as_str()).unwrap_or("-");
            println!(
                "{:<24} {:<14} {:<24} {:>4}  {}",
                n.id.as_str(),
                format!("{:?}", n.kind).to_lowercase(),
                parent,
                n.position,
                snippet
            );
        }
        println!("\ntotal: {}", nodes.len());
    }
    Ok(())
}
