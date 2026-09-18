use anyhow::{Result, anyhow};

use aidoc::storage::crud;

use crate::session::Session;

pub fn run(path: &str, node_id: &str, json: bool) -> Result<()> {
    let s = Session::open(path)?;
    let doc_id = s.package.as_ref().unwrap().manifest.document.id.clone();
    let id = aidoc::NodeId::new(node_id).map_err(|e| anyhow!("invalid node id: {e}"))?;
    let node = crud::get_node(s.store.conn(), &doc_id, &id)?
        .ok_or_else(|| anyhow!("node not found: {node_id}"))?;

    if json {
        println!("{}", serde_json::to_string_pretty(&node)?);
    } else {
        println!("id        : {}", node.id.as_str());
        println!("kind      : {:?}", node.kind);
        println!(
            "parent    : {}",
            node.parent.as_ref().map(|p| p.as_str()).unwrap_or("-")
        );
        println!("position  : {}", node.position);
        println!(
            "semantic  : {}",
            node.semantic_type.as_deref().unwrap_or("-")
        );
        println!("content   :");
        for line in node.content.lines() {
            println!("  {line}");
        }
        if !node.attributes.is_empty() {
            println!("attrs     :");
            for (k, v) in &node.attributes {
                println!("  {k} = {v}");
            }
        }
    }
    Ok(())
}
