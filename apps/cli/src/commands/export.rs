use anyhow::{Result, anyhow};

use aidoc::exporter::{self, ExportFormat, ExportInput, UnknownExportFormat};
use aidoc::storage::crud;

use crate::session::Session;

pub fn run(path: &str, out: &str, format: &str) -> Result<()> {
    let fmt: ExportFormat = format.parse().map_err(|e: UnknownExportFormat| anyhow!(e))?;

    let s = Session::open(path)?;
    let m = &s.package.as_ref().unwrap().manifest;
    let doc_id = m.document.id.clone();
    let doc = crud::get_document(s.store.conn(), &doc_id)?
        .ok_or_else(|| anyhow!("document {doc_id} missing"))?;
    let nodes = crud::list_nodes(s.store.conn(), &doc_id)?;
    let rels = crud::list_relations(s.store.conn(), &doc_id)?;
    let branch = crud::head_branch(s.store.conn(), &doc_id)?;

    // HTML re-stamps the title from the manifest; Markdown keeps the stored one.
    let mut stamped = doc.clone();
    stamped.title = m.document.title.clone();
    let doc_ref = if fmt == ExportFormat::Html {
        &stamped
    } else {
        &doc
    };

    let rendered = exporter::export(
        fmt,
        &ExportInput {
            doc: doc_ref,
            nodes: &nodes,
            relations: &rels,
            branch: branch.as_deref(),
        },
    );

    std::fs::write(out, rendered)?;
    println!("exported {format} -> {out}");
    Ok(())
}
