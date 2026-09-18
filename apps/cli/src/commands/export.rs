use anyhow::{anyhow, Result};

use aidoc::storage::crud;
use aidoc::{exporter, Document};

use crate::session::Session;

pub fn run(path: &str, out: &str, format: &str) -> Result<()> {
    let s = Session::open(path)?;
    let m = &s.package.as_ref().unwrap().manifest;
    let doc_id = m.document.id.clone();
    let doc = crud::get_document(s.store.conn(), &doc_id)?
        .ok_or_else(|| anyhow!("document {doc_id} missing"))?;
    let nodes = crud::list_nodes(s.store.conn(), &doc_id)?;
    let rels = crud::list_relations(s.store.conn(), &doc_id)?;

    let rendered = match format {
        "html" => {
            let mut d = doc.clone();
            // re-stamp title from manifest for clarity
            d.title = m.document.title.clone();
            aidoc_renderer_alt(&d, &nodes, &rels)
        }
        "md" | "markdown" => exporter::export_markdown(&doc, &nodes),
        other => return Err(anyhow!("unknown format: {other} (use html or md)")),
    };

    std::fs::write(out, rendered)?;
    println!("exported {format} -> {out}");
    Ok(())
}

// alias since aidoc::exporter doesn't take relations yet
fn aidoc_renderer_alt(doc: &Document, nodes: &[aidoc::Node], _rels: &[aidoc::Relation]) -> String {
    exporter::export_html(doc, nodes)
}