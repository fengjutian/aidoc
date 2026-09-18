use anyhow::{Context, Result};

use aidoc::storage::{AnyhowErr, crud};
use aidoc::{Document, Node, NodeId, NodeKind, OpId, Revision, RevisionId};

use crate::session::Session;

pub fn run(path: &str, doc_id: Option<String>, title: Option<String>) -> Result<()> {
    let doc_id_s = doc_id.unwrap_or_else(|| {
        std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("doc")
            .to_string()
    });
    let title_s = title.unwrap_or_else(|| doc_id_s.clone());

    let mut s = Session::create(path, &doc_id_s, &title_s)?;

    s.store
        .tx::<_, _, AnyhowErr>(|tx| {
            let doc = Document::new(
                doc_id_s.clone(),
                title_s.clone(),
                NodeId::from_validated("root"),
            );
            crud::upsert_document(tx, &doc)?;
            let mut root = Node::new(NodeId::from_validated("root"), NodeKind::Section);
            root.content = title_s.clone();
            crud::insert_node(tx, &doc_id_s, &root)?;
            Ok::<(), AnyhowErr>(())
        })
        .map_err(|e| anyhow::anyhow!("seed empty document: {e}"))?;

    let rev = Revision {
        id: RevisionId::new("R000"),
        parent: None,
        operation: OpId::new("OP-000"),
        created_at: chrono::Utc::now(),
        message: Some("initial empty revision".into()),
    };
    s.store
        .tx::<_, _, AnyhowErr>(|tx| {
            crud::insert_revision(tx, &doc_id_s, &rev, true)?;
            Ok::<(), AnyhowErr>(())
        })
        .map_err(|e| anyhow::anyhow!("insert R000: {e}"))?;

    s.save().context("save package")?;
    println!("created {path}");
    Ok(())
}
