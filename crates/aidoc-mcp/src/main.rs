//! aidoc-mcp: Model Context Protocol server over stdio.
//!
//! Implements ServerHandler directly (no macros) and dispatches `call_tool`
//! requests through a HashMap of free async functions. Every tool calls
//! into the same `aidoc` core pipeline the CLI uses, so spec MUSTs hold.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use aidoc::{
    apply_operation, create_package, export_html as core_export_html, open_package,
    revert_to, save_package, Document, Node, NodeId, Operation, OperationType, Patch, Provenance,
    Revision, RevisionId, OpId,
};
use aidoc_storage::{crud, Store};

use futures::future::BoxFuture;
use rmcp::{
    model::*,
    service::RequestContext, ErrorData, RoleServer, ServerHandler,
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

trait IntoStrErr<T> {
    fn str_err(self) -> Result<T, String>;
}
impl<T, E: std::fmt::Display> IntoStrErr<T> for Result<T, E> {
    fn str_err(self) -> Result<T, String> {
        self.map_err(|e| format!("{e}"))
    }
}

// ---------- tool functions (free async fns) ----------

async fn init_aidoc(
    state: Arc<ServerState>,
    args: serde_json::Map<String, serde_json::Value>,
) -> Result<InitResult, String> {
    let path = need::<String>(&args, "path")?;
    let doc_id = need::<String>(&args, "doc_id")?;
    let title = need::<String>(&args, "title")?;
    let (package, mut store) = create_package(std::path::PathBuf::from(&path), &doc_id, &title).str_err()?;
    seed_root_and_r000(&mut store, &doc_id, &title).str_err()?;
    let info = InitResult {
        doc_id,
        title,
        head_revision: "R000".into(),
        path,
    };
    *state.inner.lock().unwrap() = Some(Session { store, package });
    Ok(info)
}

async fn open_aidoc(
    state: Arc<ServerState>,
    args: serde_json::Map<String, serde_json::Value>,
) -> Result<InitResult, String> {
    let path = need::<String>(&args, "path")?;
    let (package, store) = open_package(std::path::PathBuf::from(&path)).str_err()?;
    let doc_id = package.manifest.document.id.clone();
    let head = crud::head_revision(store.conn(), &doc_id)
        .str_err()?
        .unwrap_or_else(|| "R000".to_string());
    let title = package.manifest.document.title.clone();
    let info = InitResult {
        doc_id,
        title,
        head_revision: head,
        path,
    };
    *state.inner.lock().unwrap() = Some(Session { store, package });
    Ok(info)
}

async fn save_aidoc(state: Arc<ServerState>) -> Result<String, String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(missing_doc)?;
    save_package(&mut s.package, &s.store).str_err()?;
    Ok("saved".to_string())
}

async fn list_nodes(state: Arc<ServerState>) -> Result<Vec<NodeDto>, String> {
    with_doc(&state, |s, doc_id| {
        let nodes = crud::list_nodes(s.store.conn(), doc_id).str_err()?;
        Ok(nodes.into_iter().map(node_to_dto).collect())
    })
}

async fn show_node(
    state: Arc<ServerState>,
    args: serde_json::Map<String, serde_json::Value>,
) -> Result<NodeDto, String> {
    let node_id = need::<String>(&args, "node_id")?;
    with_doc(&state, |s, doc_id| {
        let id = NodeId::from_validated(&node_id);
        let n = crud::get_node(s.store.conn(), doc_id, &id)
            .str_err()?
            .ok_or_else(|| format!("node not found: {node_id}"))?;
        Ok(node_to_dto(n))
    })
}

async fn create_node(
    state: Arc<ServerState>,
    args: serde_json::Map<String, serde_json::Value>,
) -> Result<RevisionDto, String> {
    let target = need::<String>(&args, "target")?;
    let content = need::<String>(&args, "content")?;
    run_op(state, OperationType::Create, target, Some(content)).await
}

async fn update_node(
    state: Arc<ServerState>,
    args: serde_json::Map<String, serde_json::Value>,
) -> Result<RevisionDto, String> {
    let target = need::<String>(&args, "target")?;
    let content = need::<String>(&args, "content")?;
    run_op(state, OperationType::Update, target, Some(content)).await
}

async fn delete_node(
    state: Arc<ServerState>,
    args: serde_json::Map<String, serde_json::Value>,
) -> Result<RevisionDto, String> {
    let target = need::<String>(&args, "target")?;
    run_op(state, OperationType::Delete, target, None).await
}

async fn apply_operation_tool(
    state: Arc<ServerState>,
    args: serde_json::Map<String, serde_json::Value>,
) -> Result<RevisionDto, String> {
    let op_json = need::<serde_json::Value>(&args, "op_json")?;
    let op: Operation = serde_json::from_value(op_json).str_err()?;
    let _doc_id = current_doc_id(&state)?;
    with_doc(&state, |s, doc_id| {
        let out = apply_operation(&mut s.store, doc_id, op).str_err()?;
        s.package.manifest.set_revision(out.revision.as_str());
        Ok(rev_to_dto(&out.revision, &out.op_id))
    })
}

async fn history(state: Arc<ServerState>) -> Result<Vec<RevisionDto>, String> {
    with_doc(&state, |s, doc_id| {
        let revs = crud::list_revisions(s.store.conn(), doc_id).str_err()?;
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
    })
}

async fn revert_tool(
    state: Arc<ServerState>,
    args: serde_json::Map<String, serde_json::Value>,
) -> Result<RevisionDto, String> {
    let target_revision = need::<String>(&args, "target_revision")?;
    with_doc(&state, |s, doc_id| {
        let out = revert_to(
            &mut s.store,
            doc_id,
            RevisionId::new(&target_revision),
            Some(format!("mcp revert to {target_revision}")),
        )
        .str_err()?;
        s.package.manifest.set_revision(out.new_revision.as_str());
        Ok(RevisionDto {
            id: out.new_revision.as_str().to_owned(),
            parent: Some(target_revision),
            operation: out.op_id.as_str().to_owned(),
            created_at: chrono::Utc::now().to_rfc3339(),
            message: Some(format!("revert to {}", out.target_revision.as_str())),
        })
    })
}

async fn export_html(state: Arc<ServerState>) -> Result<String, String> {
    with_doc(&state, |s, doc_id| {
        let doc = crud::get_document(s.store.conn(), doc_id)
            .str_err()?
            .ok_or_else(|| "document row missing".to_string())?;
        let nodes = crud::list_nodes(s.store.conn(), doc_id).str_err()?;
        Ok(core_export_html(&doc, &nodes))
    })
}

async fn validate(state: Arc<ServerState>) -> Result<String, String> {
    with_doc(&state, |s, doc_id| {
        let report = aidoc_validator::validate(&s.store, doc_id).str_err()?;
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
    })
}

// ---------- server ----------

#[derive(Clone)]
struct AIDocServer {
    state: Arc<ServerState>,
    tools: Arc<HashMap<String, ToolEntry>>,
}

struct ToolEntry {
    description: &'static str,
    handler: fn(Arc<ServerState>, serde_json::Map<String, serde_json::Value>) -> BoxFuture<'static, Result<serde_json::Value, String>>,
}

impl AIDocServer {
    fn new() -> Self {
        let state = Arc::new(ServerState::default());
        let mut tools: HashMap<String, ToolEntry> = HashMap::new();

        let mut reg = |name: &'static str, desc: &'static str, h: ToolEntry| {
            tools.insert(name.into(), ToolEntry { description: desc, handler: h.handler });
        };

        // No-arg tools: wrap the closure to discard the args map.
        let no_args = |h: fn(Arc<ServerState>) -> _| -> ToolEntry {
            ToolEntry { description: "", handler: |s, _| Box::pin(h(s)) }
        };

        // With-args tools: pass through.
        let with_args = |h: fn(Arc<ServerState>, serde_json::Map<String, serde_json::Value>) -> _| -> ToolEntry {
            ToolEntry { description: "", handler: |s, args| Box::pin(h(s, args)) }
        };

        reg("init_aidoc", "Create a new .aidoc package and open it.", with_args(init_aidoc));
        reg("open_aidoc", "Open an existing .aidoc package.", with_args(open_aidoc));
        reg("save_aidoc", "Persist the current document.", no_args(save_aidoc));
        reg("list_nodes", "List every node in the current document.", no_args(list_nodes));
        reg("show_node", "Show one node by id.", with_args(show_node));
        reg("create_node", "Create a new node with content.", with_args(create_node));
        reg("update_node", "Update a node's content.", with_args(update_node));
        reg("delete_node", "Delete a node.", with_args(delete_node));
        reg("apply_operation", "Apply a raw Operation JSON.", with_args(apply_operation_tool));
        reg("history", "List revision history.", no_args(history));
        reg("revert", "Revert to a previous revision.", with_args(revert_tool));
        reg("export_html", "Render as HTML.", no_args(export_html));
        reg("validate", "Run all validators.", no_args(validate));

        Self {
            state,
            tools: Arc::new(tools),
        }
    }
}

// ---------- ServerHandler impl ----------

impl ServerHandler for AIDocServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            server_info: Implementation {
                name: "aidoc-mcp".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some(
                "AIDoc v0.1 MCP server. Initialize or open a .aidoc file first, then list / show / update / revert / export nodes."
                    .into(),
            ),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = self
            .tools
            .iter()
            .map(|(name, entry)| {
                Tool::new(
                    name.clone(),
                    Some(entry.description.into()),
                    EmptyInputSchema,
                )
            })
            .collect();
        Ok(ListToolsResult { tools, ..Default::default() })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let name = request.name.to_string();
        let entry = match self.tools.get(&name) {
            Some(e) => e,
            None => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "unknown tool: {name}"
                ))]));
            }
        };
        let args = match request.arguments {
            Some(map) => (*map).clone(),
            None => serde_json::Map::new(),
        };
        let state = self.state.clone();
        match (entry.handler)(state, args).await {
            Ok(serde_json::Value::Null) => {
                Ok(CallToolResult::success(vec![Content::text("ok")]))
            }
            Ok(v) => match serde_json::to_string(&v) {
                Ok(s) => Ok(CallToolResult::success(vec![Content::text(s)])),
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                    "serialize: {e}"
                ))])),
            },
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
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

fn need<T: serde::de::DeserializeOwned>(
    args: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<T, String> {
    let v = args.get(key).ok_or_else(|| format!("missing arg: {key}"))?;
    serde_json::from_value(v.clone()).str_err()
}

async fn run_op(
    state: Arc<ServerState>,
    op_type: OperationType,
    target: String,
    content: Option<String>,
) -> Result<RevisionDto, String> {
    let doc_id = current_doc_id(&state)?;
    let head = with_doc(&state, |s, doc_id| {
        Ok(crud::head_revision(s.store.conn(), doc_id)
            .str_err()?
            .ok_or_else(|| "no head revision".to_string())?)
    })?;

    let op = Operation {
        id: OpId::new(format!("OP-{}", chrono::Utc::now().timestamp_millis())),
        op_type,
        target: Some(NodeId::from_validated(&target)),
        expected_revision: RevisionId::new(head),
        target_revision: None,
        targets: vec![],
        actor: Provenance::ai("mcp".to_string(), None),
        patch: content.map(|c| Patch {
            content: Some(c),
            ..Default::default()
        }),
        reason: Some(format!("mcp {op_type:?} {target}")),
    };

    with_doc(&state, |s, doc_id| {
        let out = apply_operation(&mut s.store, doc_id, op).str_err()?;
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
    let transport = rmcp::transport::stdio();
    let svc = server.serve(transport).await?;
    svc.waiting().await?;
    Ok(())
}