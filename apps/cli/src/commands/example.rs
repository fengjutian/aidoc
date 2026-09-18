//! `aidoc example` — scaffold one of the bundled example documents.

use anyhow::{Context, Result};

use aidoc::apply_operation;
use aidoc::id::{NodeId, OpId, RevisionId};
use aidoc::model::{Operation, OperationType, Patch, Provenance};
use aidoc::storage::{AnyhowErr, crud};

use crate::session::Session;

/// A node template: id + content (raw text). Parent / position defaults to
/// root for everything; the example walks the tree in dependency order so
/// CREATE-then-UPDATE captures the layered feel.
#[derive(Clone)]
struct TmplNode {
    id: &'static str,
    content: &'static str,
}

fn order_system_nodes() -> Vec<TmplNode> {
    vec![
        TmplNode {
            id: "root",
            content: "订单系统技术文档",
        },
        TmplNode {
            id: "overview",
            content: "本系统负责订单全生命周期管理：下单、库存扣减、支付、履约。",
        },
        TmplNode {
            id: "requirements",
            content: "REQ-001: 系统必须支持订单创建、查询、取消。\nREQ-002: 订单状态变更必须可追溯。",
        },
        TmplNode {
            id: "decision-microservices",
            content: "采用微服务架构：order-service、inventory-service、payment-service 独立部署。",
        },
        TmplNode {
            id: "problem-n-plus-one",
            content: "订单查询接口存在 N+1 查询问题：先查订单列表，再循环查每笔订单的明细。",
        },
        TmplNode {
            id: "solution-batch-query",
            content: "使用批量查询替代逐条查询：一次 SQL 拉所有订单及其明细。\n引入 DataLoader 在 Service 层做聚合。",
        },
        TmplNode {
            id: "code-ref-order-service",
            content: "订单服务负责订单创建和查询。",
        },
        TmplNode {
            id: "diagram-order-flow",
            content: "graph TD\n  A[创建订单] --> B[库存检查]\n  B --> C[扣库存]\n  C --> D[订单创建]\n  D --> E[返回结果]",
        },
    ]
}

fn api_system_nodes() -> Vec<TmplNode> {
    vec![
        TmplNode {
            id: "root",
            content: "REST API 设计规范",
        },
        TmplNode {
            id: "overview",
            content: "公司内部 REST API 统一规范，覆盖版本、鉴权、错误处理。",
        },
        TmplNode {
            id: "versioning",
            content: "URL 路径版本化：/api/v1/orders, /api/v2/orders。主版本下线前至少公告 6 个月。",
        },
        TmplNode {
            id: "auth",
            content: "JWT Bearer Token。Access Token 15 分钟，Refresh Token 7 天。",
        },
        TmplNode {
            id: "errors",
            content: "统一错误响应 { code, details }。HTTP 状态码必须准确：404 vs 422。",
        },
        TmplNode {
            id: "rate-limit",
            content: "每用户 100 req/min，超限返回 429。",
        },
    ]
}

fn knowledge_graph_nodes() -> Vec<TmplNode> {
    vec![
        TmplNode {
            id: "root",
            content: "Knowledge Graph for Code Wiki",
        },
        TmplNode {
            id: "overview",
            content: "AIDoc Node graph backed by SQLite. Relations enable structure-aware RAG. ",
        },
        TmplNode {
            id: "requirement-rag",
            content: "RAG chunks must carry node_id, parent_id, revision, content_hash, relations.",
        },
        TmplNode {
            id: "diagram-graph",
            content: "graph LR\n  Document --> Node\n  Node --> Relation\n  Node --> CodeRef\n  CodeRef --> Provenance",
        },
        TmplNode {
            id: "code-ref-vector-store",
            content: "Vector store keys by node_id; relations + parent expand the retrieval context.",
        },
    ]
}

pub fn run(template: &str, path: &str, export_html: Option<&str>) -> Result<()> {
    let nodes: &[TmplNode] = match template {
        "order-system" => order_system_nodes().leak(),
        "api-system" => api_system_nodes().leak(),
        "knowledge-graph" => knowledge_graph_nodes().leak(),
        other => anyhow::bail!(
            "unknown template: {other} (try order-system / api-system / knowledge-graph)"
        ),
    };

    let doc_id = template.replace('-', "_");
    let title = nodes.first().map(|n| n.content).unwrap_or(template);

    // 1. Create empty package + root node.
    let mut s = Session::create(path, &doc_id, title)?;
    s.store
        .tx::<_, _, AnyhowErr>(|tx| {
            let doc = aidoc::Document::new(
                doc_id.clone(),
                title.to_string(),
                NodeId::from_validated("root"),
            );
            crud::upsert_document(tx, &doc)?;
            Ok::<_, AnyhowErr>(())
        })
        .map_err(|e| anyhow::anyhow!("init: {e}"))?;

    // 2. Stamp the synthetic R000 so CREATE ops can reference it as expected_revision.
    let rev = aidoc::Revision {
        id: RevisionId::new("R000"),
        parent: None,
        operation: OpId::new("OP-000"),
        created_at: chrono::Utc::now(),
        message: Some(format!("example:{template} seed")),
        branch: None,
    };
    s.store
        .tx::<_, _, AnyhowErr>(|tx| {
            crud::advance_head(tx, &doc_id, "R000").ok();
            crud::insert_revision(tx, &doc_id, &rev, true)?;
            Ok::<_, AnyhowErr>(())
        })
        .map_err(|e| anyhow::anyhow!("seed R000: {e}"))?;

    // 3. CREATE every non-root node with its template content. Each apply advances
    //    the document head, so we re-read after every op and feed the new head
    //    to the next expected_revision.
    let mut head = RevisionId::new("R000");
    for tpl in nodes.iter().skip(1) {
        let op = Operation {
            id: OpId::new(format!("OP-seed-{}", tpl.id)),
            op_type: OperationType::Create,
            target: Some(NodeId::from_validated(tpl.id)),
            expected_revision: head.clone(),
            expected_hash: None,
            target_revision: None,
            targets: vec![],
            actor: Provenance::human(Some(format!("example:{template}"))),
            patch: Some(Patch {
                kind: None,
                position: None,
                content: Some(tpl.content.to_string()),
                ..Default::default()
            }),
            reason: Some(format!("scaffold {}/{}", template, tpl.id)),
        };
        apply_operation(&mut s.store, &doc_id, op)
            .map_err(|e| anyhow::anyhow!("create {}: {e}", tpl.id))?;
        // After apply, read the actual current head (R001 after first CREATE, etc).
        let current_head = aidoc::storage::crud::head_revision(s.store.conn(), &doc_id)
            .map_err(|e| anyhow::anyhow!("head_revision: {e}"))?
            .unwrap_or_else(|| "R000".to_string());
        head = RevisionId::new(current_head);
    }

    // 4. Persist + (optional) HTML export.
    if let Some(pkg) = s.package.as_mut() {
        let last = aidoc::storage::crud::list_revisions(s.store.conn(), &doc_id)
            .map_err(|e| anyhow::anyhow!("list revisions: {e}"))?;
        if let Some(head) = last.last() {
            pkg.manifest.set_revision(head.id.as_str());
        }
    }
    let rev_count = aidoc::storage::crud::list_revisions(s.store.conn(), &doc_id)
        .map_err(|e| anyhow::anyhow!("list revisions: {e}"))?
        .len();
    s.save().context("save package")?;
    println!(
        "wrote {} ({} nodes, {} revisions)",
        path,
        nodes.len(),
        rev_count
    );

    if let Some(out_html) = export_html {
        let s2 = Session::open(path)?;
        let doc = aidoc::storage::crud::get_document(s2.store.conn(), &doc_id)
            .map_err(|e| anyhow::anyhow!("get doc: {e}"))?
            .ok_or_else(|| anyhow::anyhow!("doc missing"))?;
        let all_nodes = aidoc::storage::crud::list_nodes(s2.store.conn(), &doc_id)
            .map_err(|e| anyhow::anyhow!("list nodes: {e}"))?;
        let html = aidoc::exporter::export_html(&doc, &all_nodes, None);
        std::fs::write(out_html, html).context("write html")?;
        println!("exported HTML → {out_html}");
    }
    Ok(())
}
