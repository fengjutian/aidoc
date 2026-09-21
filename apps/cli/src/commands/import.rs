//! `aidoc import-json <out.aidoc> <in.json>` — create a brand-new package
//! from a v0.2 canonical document JSON. The JSON is validated against
//! `document.schema.json`; unknown node kinds are mapped to
//! `NodeKind::Generic` with the raw value preserved in `semantic_type`.

use anyhow::{Context, Result};

use aidoc::storage::{AnyhowErr, crud};
use aidoc::{Document, Node, NodeId, NodeKind, OpId, Revision, RevisionId, import_canonical_document};
use serde_json::Value;

use crate::session::Session;

pub fn run(out: &str, json: &str) -> Result<()> {
    // Peek the JSON to seed the package with the right doc_id/title.
    let raw = std::fs::read_to_string(json).with_context(|| format!("read {json}"))?;
    let parsed: Value = serde_json::from_str(&raw).with_context(|| format!("parse {json}"))?;
    let doc_id = parsed
        .get("document")
        .and_then(|d| d.get("id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("canonical JSON is missing document.id"))?
        .to_string();
    let title = parsed
        .get("document")
        .and_then(|d| d.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or(&doc_id)
        .to_string();

    let mut session = Session::create(out, &doc_id, &title)?;
    seed_initial(&mut session, &doc_id, &title)?;
    let head = import_canonical_document(
        session.package.as_ref().expect("just created"),
        &mut session.store,
        json,
    )
    .map_err(|e| anyhow::anyhow!("import: {e}"))?;

    session.save().context("save package")?;
    println!("imported {json} -> {out} (head = {head})");
    Ok(())
}

fn seed_initial(session: &mut Session, doc_id: &str, title: &str) -> Result<()> {
    session
        .store
        .tx::<_, _, AnyhowErr>(|tx| {
            let doc = Document::new(
                doc_id.to_string(),
                title.to_string(),
                NodeId::from_validated("root"),
            );
            crud::upsert_document(tx, &doc)?;
            let mut root = Node::new(NodeId::from_validated("root"), NodeKind::Section);
            root.content = title.to_string();
            crud::insert_node(tx, doc_id, &root)?;
            Ok::<(), AnyhowErr>(())
        })
        .map_err(|e| anyhow::anyhow!("seed empty document: {e}"))?;

    let rev = Revision {
        id: RevisionId::new("R000"),
        parent: None,
        operation: OpId::new("OP-000"),
        created_at: chrono::Utc::now(),
        message: Some("seed before canonical import".into()),
        branch: None,
    };
    session
        .store
        .tx::<_, _, AnyhowErr>(|tx| {
            crud::insert_revision(tx, doc_id, &rev, true)?;
            Ok::<(), AnyhowErr>(())
        })
        .map_err(|e| anyhow::anyhow!("insert R000: {e}"))?;
    Ok(())
}