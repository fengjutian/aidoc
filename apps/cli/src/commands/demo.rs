//! Demo: runs the full create → update → revert loop on a brand-new file
//! and dumps the resulting HTML.
//!
//! `aidoc demo path/to/foo.aidoc --export-html foo.html`

use anyhow::{Context, Result};

use aidoc::id::NodeId;
use aidoc::storage::{AnyhowErr, crud};
use aidoc::{NodeKind, OpId, Operation, OperationType, Patch, Provenance, Revision, RevisionId};

use crate::session::Session;

pub fn run(path: &str, export_html: Option<&str>) -> Result<()> {
    let doc_id = "demo-doc";
    let title = "AIDoc v0.1 Demo";

    let mut s = Session::create(path, doc_id, title)?;

    s.store
        .tx::<_, _, AnyhowErr>(|tx| {
            let doc = aidoc::Document::new(
                doc_id.to_string(),
                title.to_string(),
                NodeId::from_validated("root"),
            );
            crud::upsert_document(tx, &doc)?;
            let mut root = aidoc::Node::new(NodeId::from_validated("root"), NodeKind::Section);
            root.content = title.to_string();
            crud::insert_node(tx, doc_id, &root)?;
            Ok::<_, AnyhowErr>(())
        })
        .map_err(|e| anyhow::anyhow!("seed: {e}"))?;

    let rev0 = Revision {
        id: RevisionId::new("R000"),
        parent: None,
        operation: OpId::new("OP-000"),
        created_at: chrono::Utc::now(),
        message: Some("initial empty revision".into()),
        branch: None,
    };
    s.store
        .tx::<_, _, AnyhowErr>(|tx| {
            crud::insert_revision(tx, doc_id, &rev0, true)?;
            Ok::<_, AnyhowErr>(())
        })
        .map_err(|e| anyhow::anyhow!("R000: {e}"))?;

    // CREATE an "architecture" node.
    let head0 =
        crud::head_revision(s.store.conn(), doc_id)?.ok_or_else(|| anyhow::anyhow!("no head"))?;
    let create_op = Operation {
        id: OpId::new("OP-001"),
        op_type: OperationType::Create,
        target: Some(NodeId::from_validated("architecture")),
        expected_revision: RevisionId::new(head0),
        target_revision: None,
        targets: vec![],
        actor: Provenance::human(Some("demo".into())),
        patch: Some(Patch {
            content: Some("系统采用微服务架构。".into()),
            ..Default::default()
        }),
        reason: Some("add architecture section".into()),
    };
    let out1 = aidoc::operation::apply_operation(&mut s.store, doc_id, create_op)
        .map_err(|e| anyhow::anyhow!("create: {e}"))?;
    println!("after CREATE  : head = {}", out1.revision.as_str());

    // UPDATE that node.
    let update_op = Operation {
        id: OpId::new("OP-002"),
        op_type: OperationType::Update,
        target: Some(NodeId::from_validated("architecture")),
        expected_revision: out1.revision.clone(),
        target_revision: None,
        targets: vec![],
        actor: Provenance::ai("document-agent".to_string(), Some("MiniMax-M3".to_string())),
        patch: Some(Patch {
            content: Some("系统采用微服务架构，包含订单、库存、支付三个核心服务。".into()),
            ..Default::default()
        }),
        reason: Some("expand architecture description".into()),
    };
    let out2 = aidoc::operation::apply_operation(&mut s.store, doc_id, update_op)
        .map_err(|e| anyhow::anyhow!("update: {e}"))?;
    println!("after UPDATE  : head = {}", out2.revision.as_str());

    // REVERT to the post-create revision.
    let rev_outcome = aidoc::history::revert_to(
        &mut s.store,
        doc_id,
        out1.revision.clone(),
        Some("demo: roll back the UPDATE".into()),
    )
    .map_err(|e| anyhow::anyhow!("revert: {e}"))?;
    println!(
        "after REVERT  : new head = {} (reverted to {})",
        rev_outcome.new_revision.as_str(),
        rev_outcome.target_revision.as_str()
    );

    if let Some(pkg) = s.package.as_mut() {
        pkg.manifest.set_revision(rev_outcome.new_revision.as_str());
    }
    s.save().context("save package")?;

    println!("\ndemo done → {path}");

    if let Some(html_out) = export_html {
        let s2 = Session::open(path)?;
        let doc = crud::get_document(s2.store.conn(), doc_id)?
            .ok_or_else(|| anyhow::anyhow!("doc missing"))?;
        let nodes = crud::list_nodes(s2.store.conn(), doc_id)?;
        let html = aidoc::exporter::export_html(&doc, &nodes);
        std::fs::write(html_out, html)?;
        println!("exported HTML → {html_out}");
    }
    Ok(())
}
