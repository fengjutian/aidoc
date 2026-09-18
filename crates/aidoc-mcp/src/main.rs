//! aidoc-mcp: Model Context Protocol server over stdio.
//!
//! Each tool is a thin wrapper around an aidoc core call. The server keeps
//! exactly one open document per session, switched by init_aidoc /
//! open_aidoc. Writes go through the same apply_operation pipeline the
//! CLI uses, so every spec MUST holds.

use std::sync::Mutex;

use aidoc::{
    apply_operation, create_package, export_html as core_export_html, open_package,
    revert_to, save_package, Document, Node, NodeId, Operation, OperationType, Patch, Provenance,
    Revision, RevisionId, OpId,
};
use aidoc_storage::{crud, Store};

use rmcp::{
    handler::server::tool::ToolRouter, model::*, tool, tool_handler, tool_router, transport::stdio,
    Error as McpError, ServiceExt,
};
use serde::Serialize;

// ---------- shared state ----------

#[derive(Default)]
struct ServerState {
    inner: Mutex<Option<Session>>,
}

struct Session {
    store: Store,
    package: aidoc::Package,
}

#[derive(Debug, Serialize)]
struct NodeDto {
    id: String,
    kind: String,
    parent: Option<String>,
    position: u32,
    content: String,
}

#[derive(Debug, Serialize)]
struct RevisionDto {
    id: String,
    parent: Option<String>,
    operation: String,
    created_at: String,
    message: Option<String>,
}

#[derive(Debug, Serialize)]
struct InitResult {
    doc_id: String,
    title: String,
    head_revision: String,
    path: String,
}

// ---------- result helpers ----------

fn result_to_mcp<T: Serialize, E: std::fmt::Display>(r: Result<T, E>) -> Result<CallToolResult, McpError> {
    match r {
        Ok(v) => {
            let json = serde_json::to_string(&v)
                .map_err(|e| McpError::msg(format!("serialize: {e}")))?;
            Ok(CallToolResult::success(vec![Content::text(json)]))
        }
        Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
    }
}

fn string_to_mcp<E: std::fmt::Display>(r: Result<String, E>) -> Result<CallToolResult, McpError> {
    result_to_mcp(r)
}

// ---------- server ----------

#[derive(Clone)]
struct AIDocServer {
    state: std::sync::Arc<ServerState>,
    tool_router: ToolRouter<Self>,
}

#[tool_handler(router = self.tool_router)]
impl rmcp::ServerHandler for AIDocServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            server_info: Implementation {
                name: "aidoc-mcp".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some(
                "AIDoc v0.1 MCP server. Initialize or open a .aidoc file first, then list / show / update / revert / export nodes. \
                 All writes go through apply_operation so every change is a new revision and revert creates a new revision too."
                    .into(),
            ),
        }
    }
}

#[tool_router]
impl AIDocServer {
    fn new() -> Self {
        Self {
            state: std::sync::Arc::new(ServerState::default()),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Create a brand-new .aidoc package on disk and open it.")]
    async fn init_aidoc(
        &self,
        path: String,
        doc_id: String,
        title: String,
    ) -> Result<CallToolResult, McpError> {
        let r: Result<InitResult, String> = (|| {
            let (package, mut store) =
                create_package(std::path::PathBuf::from(&path), &doc_id, &title)?;
            seed_root_and_r000(&mut store, &doc_id, &title)?;
            let info = InitResult {
                doc_id,
                title,
                head_revision: "R000".into(),
                path,
            };
            *self.state.inner.lock().unwrap() = Some(Session { store, package });
            Ok(info)
        })();
        result_to_mcp(r)
    }

    #[tool(description = "Open an existing .aidoc package from disk.")]
    async fn open_aidoc(&self, path: String) -> Result<CallToolResult, McpError> {
        let r: Result<InitResult, String> = (|| {
            let (package, store) = open_package(std::path::PathBuf::from(&path))?;
            let doc_id = package.manifest.document.id.clone();
            let head = crud::head_revision(store.conn(), &doc_id)?
                .unwrap_or_else(|| "R000".to_string());
            let title = package.manifest.document.title.clone();
            let info = InitResult {
                doc_id,
                title,
                head_revision: head,
                path,
            };
            *self.state.inner.lock().unwrap() = Some(Session { store, package });
            Ok(info)
        })();
        result_to_mcp(r)
    }

    #[tool(description = "Close the current document and forget its state.")]
    async fn close_aidoc(&self) -> Result<CallToolResult, McpError> {
        *self.state.inner.lock().unwrap() = None;
        result_to_mcp(Ok::<_, String>("closed".to_string()))
    }

    #[tool(description = "Save the current document back to its .aidoc ZIP file.")]
    async fn save_aidoc(&self) -> Result<CallToolResult, McpError> {
        let r: Result<String, String> = (|| {
            let mut g = self.state.inner.lock().unwrap();
            let s = g.as_mut().ok_or_else(missing_doc)?;
            save_package(&mut s.package, &s.store)?;
            Ok("saved".to_string())
        })();
        string_to_mcp(r)
    }

    #[tool(description = "List every node in the current document.")]
    async fn list_nodes(&self) -> Result<CallToolResult, McpError> {
        let r: Result<Vec<NodeDto>, String> = with_doc(&self.state, |s, doc_id| {
            let nodes = crud::list_nodes(s.store.conn(), doc_id)?;
            Ok(nodes.into_iter().map(node_to_dto).collect())
        });
        result_to_mcp(r)
    }

    #[tool(description = "Show details for one node by id.")]
    async fn show_node(&self, node_id: String) -> Result<CallToolResult, McpError> {
        let r: Result<NodeDto, String> = with_doc(&self.state, |s, doc_id| {
            let id = NodeId::from_validated(&node_id);
            let n = crud::get_node(s.store.conn(), doc_id, &id)?
                .ok_or_else(|| format!("node not found: {node_id}"))?;
            Ok(node_to_dto(n))
        });
        result_to_mcp(r)
    }

    #[tool(description = "Update a node's content. Wraps a typed Update Operation, so a new revision is produced and conflict-checked against the current head.")]
    async fn update_node(
        &self,
        target: String,
        content: String,
    ) -> Result<CallToolResult, McpError> {
        result_to_mcp(run_op(&self.state, OperationType::Update, target, Some(content)))
    }

    #[tool(description = "Create a brand-new node. content becomes the initial text.")]
    async fn create_node(
        &self,
        target: String,
        content: String,
    ) -> Result<CallToolResult, McpError> {
        result_to_mcp(run_op(&self.state, OperationType::Create, target, Some(content)))
    }

    #[tool(description = "Delete a node by id.")]
    async fn delete_node(&self, target: String) -> Result<CallToolResult, McpError> {
        result_to_mcp(run_op(&self.state, OperationType::Delete, target, None))
    }

    #[tool(description = "Apply a raw Operation JSON object. Use this for op types not covered by the typed helpers (split, merge, move, link, unlink, etc.).")]
    async fn apply_operation(
        &self,
        op_json: serde_json::Value,
    ) -> Result<CallToolResult, McpError> {
        let r: Result<RevisionDto, String> = (|| {
            let op: Operation = serde_json::from_value(op_json)?;
            let doc_id = current_doc_id(&self.state)?;
            with_doc(&self.state, |s, doc_id| {
                let out = apply_operation(&mut s.store, doc_id, op)?;
                s.package.manifest.set_revision(out.revision.as_str());
                Ok(rev_to_dto(&out.revision, &out.op_id))
            })
        })();
        result_to_mcp(r)
    }

    #[tool(description = "List revision history, oldest first.")]
    async fn history(&self) -> Result<CallToolResult, McpError> {
        let r: Result<Vec<RevisionDto>, String> = with_doc(&self.state, |s, doc_id| {
            let revs = crud::list_revisions(s.store.conn(), doc_id)?;
            Ok(revs
                .iter()
                .map(|r| RevisionDto {
                    id: r.id.as_str().to_owned(),
                    parent: r.parent.as_ref().map(|p| p.as_str().to_owned()),
                    operation: r.operation.as_str().to_owned(),
                    created_at: r.created_at.to_rfc3339(),
                    message: r.message.clone(),
                })
                .collect())
        });
        result_to_mcp(r)
    }

    #[tool(description = "Revert to a previous revision. Always creates a new revision; old revisions are kept (spec MUST 4).")]
    async fn revert(&self, target_revision: String) -> Result<CallToolResult, McpError> {
        let r: Result<RevisionDto, String> = (|| {
            let _ = current_doc_id(&self.state)?;
            with_doc(&self.state, |s, doc_id| {
                let out = revert_to(
                    &mut s.store,
                    doc_id,
                    RevisionId::new(&target_revision),
                    Some(format!("mcp revert to {target_revision}")),
                )?;
                s.package.manifest.set_revision(out.new_revision.as_str());
                Ok(RevisionDto {
                    id: out.new_revision.as_str().to_owned(),
                    parent: Some(target_revision),
                    operation: out.op_id.as_str().to_owned(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    message: Some(format!("revert to {}", out.target_revision.as_str())),
                })
            })
        })();
        result_to_mcp(r)
    }

    #[tool(description = "Render the current document as standalone HTML.")]
    async fn export_html(&self) -> Result<CallToolResult, McpError> {
        let r: Result<String, String> = with_doc(&self.state, |s, doc_id| {
            let doc = crud::get_document(s.store.conn(), doc_id)?
                .ok_or_else(|| "document row missing".to_string())?;
            let nodes = crud::list_nodes(s.store.conn(), doc_id)?;
            Ok(core_export_html(&doc, &nodes))
        });
        string_to_mcp(r)
    }

    #[tool(description = "Run validators (identity / structure / relation / revision / code-ref) and return a short report.")]
    async fn validate(&self) -> Result<CallToolResult, McpError> {
        let r: Result<String, String> = with_doc(&self.state, |s, doc_id| {
            let report = aidoc_validator::validate(&s.store, doc_id)?;
            let mut out = String::new();
            let total = report.total_errors();
            if total == 0 {
                out.push_str("OK -- 0 errors across identity/structure/relation/revision/code-ref");
                return Ok(out);
            }
            for (label, errs) in [
                ("identity", &report.identity_errors),
                ("structure", &report.structure_errors),
                ("relation", &report.relation_errors),
                ("revision", &report.revision_errors),
                ("code-ref", &report.code_ref_errors),
            ] {
                if errs.is_empty() {
                    continue;
                }
                out.push_str(&format!("[{label}] {} error(s):\n", errs.len()));
                for e in errs {
                    out.push_str(&format!("  - {e}\n"));
                }
            }
            out.push_str(&format!("\ntotal: {total}"));
            Ok(out)
        });
        string_to_mcp(r)
    }
}

// ---------- helpers ----------

fn missing_doc() -> String {
    "no document open - call init_aidoc / open_aidoc first".to_string()
}

fn current_doc_id(state: &ServerState) -> Result<String, String> {
    let g = state.inner.lock().unwrap();
    let s = g.as_ref().ok_or_else(missing_doc)?;
    Ok(s.package.manifest.document.id.clone())
}

fn with_doc<F, T>(state: &ServerState, f: F) -> Result<T, String>
where
    F: FnOnce(&mut Session, &str) -> Result<T, String>,
{
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(missing_doc)?;
    let doc_id = s.package.manifest.document.id.clone();
    f(s, &doc_id)
}

fn run_op(
    state: &ServerState,
    op_type: OperationType,
    target: String,
    content: Option<String>,
) -> Result<RevisionDto, String> {
    let doc_id = current_doc_id(state)?;
    let head = with_doc(state, |s, doc_id| {
        Ok(crud::head_revision(s.store.conn(), doc_id)?
            .ok_or_else(|| "no head revision".to_string())?)
    })?;

    let op = Operation {
        id: OpId::new(format!("OP-{}", chrono::Utc::now().timestamp_millis())),
        op_type,
        target: Some(NodeId::from_validated(&target)),
        expected_revision: RevisionId::new(head),
        target_revision: None,
        targets: vec![],
        actor: Provenance::ai("mcp".into(), None),
        patch: content.map(|c| Patch {
            content: Some(c),
            ..Default::default()
        }),
        reason: Some(format!("mcp {op_type:?} {target}")),
    };

    with_doc(state, |s, doc_id| {
        let out = apply_operation(&mut s.store, doc_id, op)?;
        s.package.manifest.set_revision(out.revision.as_str());
        Ok(rev_to_dto(&out.revision, &out.op_id))
    })
}

fn node_to_dto(n: Node) -> NodeDto {
    NodeDto {
        id: n.id.as_str().to_owned(),
        kind: format!("{:?}", n.kind).to_lowercase(),
        parent: n.parent.as_ref().map(|p| p.as_str().to_owned()),
        position: n.position,
        content: n.content,
    }
}

fn rev_to_dto(rev: &RevisionId, op_id: &OpId) -> RevisionDto {
    RevisionDto {
        id: rev.as_str().to_owned(),
        parent: None,
        operation: op_id.as_str().to_owned(),
        created_at: chrono::Utc::now().to_rfc3339(),
        message: None,
    }
}

fn seed_root_and_r000(store: &mut Store, doc_id: &str, title: &str) -> anyhow::Result<()> {
    use aidoc::NodeKind;
    use aidoc_storage::AnyhowErr;
    store.tx::<_, _, AnyhowErr>(|tx| {
        let doc = Document::new(doc_id.to_string(), title.to_string(), NodeId::from_validated("root"));
        crud::upsert_document(tx, &doc)?;
        let mut root = Node::new(NodeId::from_validated("root"), NodeKind::Section);
        root.content = title.to_string();
        crud::insert_node(tx, doc_id, &root)?;
        let rev = Revision {
            id: RevisionId::new("R000"),
            parent: None,
            operation: OpId::new("OP-000"),
            created_at: chrono::Utc::now(),
            message: Some("mcp seed".into()),
        };
        crud::insert_revision(tx, doc_id, &rev, true)?;
        Ok::<_, AnyhowErr>(())
    })?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server = AIDocServer::new();
    let transport = stdio();
    let svc = server.serve(transport).await?;
    svc.waiting().await?;
    Ok(())
}