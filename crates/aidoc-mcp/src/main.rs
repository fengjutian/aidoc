//! aidoc-mcp: Model Context Protocol server over stdio.
//!
//! Implements ServerHandler directly (no macros) and dispatches `call_tool`
//! requests through a HashMap of free async functions. Every tool calls
//! into the same `aidoc` core pipeline the CLI uses, so spec MUSTs hold.

// Tool registry uses explicit `|s, a| foo(s, a)` closures — reads clearly as
// a dispatch table and avoids footguns around arg arity.
#![allow(clippy::redundant_closure)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use aidoc::{
    Document, Node, NodeId, OpId, Operation, OperationType, Patch, Provenance, Revision,
    RevisionId, apply_operation, create_package, export_html as core_export_html, open_package,
    revert_to, save_package,
};
use aidoc_storage::{Store, crud};

use futures::future::BoxFuture;
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::*,
    service::{RequestContext, ServiceExt},
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
    /// Named branch this revision belongs to (None = main).
    branch: Option<String>,
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
    let (mut package, mut store) =
        create_package(std::path::PathBuf::from(&path), &doc_id, &title).str_err()?;
    seed_root_and_r000(&mut store, &doc_id, &title).str_err()?;
    save_package(&mut package, &store).str_err()?;
    // save_package consumes the temp dir into the target .aidoc zip on disk;
    // we keep the package in memory for subsequent tool calls.
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
        let mut nodes = crud::list_nodes(s.store.conn(), doc_id).str_err()?;
        aidoc::inline_image_assets(&s.package, &mut nodes).str_err()?;
        Ok(nodes.into_iter().map(node_to_dto).collect())
    })
}

async fn search_nodes(
    state: Arc<ServerState>,
    args: serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<NodeDto>, String> {
    let query = need::<String>(&args, "query")?;
    let limit = opt::<usize>(&args, "limit").unwrap_or(20).max(1);
    with_doc(&state, |s, doc_id| {
        let nodes = crud::search_nodes(s.store.conn(), doc_id, &query, limit).str_err()?;
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

async fn history(
    state: Arc<ServerState>,
    args: serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<RevisionDto>, String> {
    let branch: Option<String> = args
        .get("branch")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .filter(|s| !s.is_empty());
    with_doc(&state, |s, doc_id| {
        let revs = match &branch {
            Some(name) => crud::list_revisions_by_branch(s.store.conn(), doc_id, name).str_err()?,
            None => crud::list_revisions(s.store.conn(), doc_id).str_err()?,
        };
        Ok(revs
            .iter()
            .map(|r| RevisionDto {
                id: r.id.as_str().to_owned(),
                parent: r.parent.as_ref().map(|p| p.as_str().to_owned()),
                operation: r.operation.as_str().to_owned(),
                created_at: r.created_at.to_rfc3339(),
                message: r.message.clone(),
                branch: r.branch.clone(),
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
            branch: None,
        })
    })
}

async fn export_html(state: Arc<ServerState>) -> Result<String, String> {
    with_doc(&state, |s, doc_id| {
        let doc = crud::get_document(s.store.conn(), doc_id)
            .str_err()?
            .ok_or_else(|| "document row missing".to_string())?;
        let nodes = crud::list_nodes(s.store.conn(), doc_id).str_err()?;
        let branch = crud::head_branch(s.store.conn(), doc_id).str_err()?;
        Ok(core_export_html(&doc, &nodes, branch.as_deref()))
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
        for cat in aidoc_validator::ValidationCategory::all() {
            let errs = report.by_category(cat);
            if errs.is_empty() {
                continue;
            }
            out.push_str(&format!("[{}] {} error(s):\n", cat.as_str(), errs.len()));
            for f in errs {
                out.push_str(&format!("  - {}\n", f.message));
            }
        }
        out.push_str(&format!("\ntotal: {total}"));
        Ok(out)
    })
}

#[derive(Debug, serde::Serialize)]
struct BranchResult {
    branch: String,
    parent: String,
    new_revision: String,
}

#[derive(Debug, serde::Serialize)]
struct MergeResult {
    branch: String,
    parent: String,
    new_revision: String,
}

async fn branch(
    state: Arc<ServerState>,
    args: serde_json::Map<String, serde_json::Value>,
) -> Result<BranchResult, String> {
    let name: String = need(&args, "name")?;
    let reason: Option<String> = args
        .get("reason")
        .and_then(|v| v.as_str().map(|s| s.to_string()));
    if name.trim().is_empty() || name == "main" {
        return Err(format!(
            "branch name must be non-empty and not 'main' (got {name:?})"
        ));
    }

    with_doc(&state, |s, doc_id| {
        let head = crud::head_revision(s.store.conn(), doc_id)
            .str_err()?
            .ok_or_else(|| "no head revision".to_string())?;
        let mut attrs = indexmap::IndexMap::new();
        attrs.insert("branch".into(), name.clone());
        let op = Operation {
            id: OpId::new(format!("OP-mcp-branch-{name}")),
            op_type: OperationType::Branch,
            target: None,
            expected_revision: RevisionId::new(head.clone()),
            expected_hash: None,
            target_revision: None,
            targets: vec![],
            actor: Provenance::human(Some("mcp".into())),
            patch: Some(Patch {
                content: None,
                title: None,
                semantic_type: None,
                kind: None,
                position: None,
                attributes: attrs,
            }),
            reason: reason.clone(),
        };
        let outcome =
            apply_operation(&mut s.store, doc_id, op).map_err(|e| format!("branch: {e}"))?;
        Ok(BranchResult {
            branch: name,
            parent: head,
            new_revision: outcome.revision.as_str().to_string(),
        })
    })
}

async fn merge(
    state: Arc<ServerState>,
    args: serde_json::Map<String, serde_json::Value>,
) -> Result<MergeResult, String> {
    let branch: String = need(&args, "branch")?;
    let reason: Option<String> = args
        .get("reason")
        .and_then(|v| v.as_str().map(|s| s.to_string()));
    if branch.trim().is_empty() || branch == "main" {
        return Err("merge source must be non-empty and not 'main'".into());
    }

    with_doc(&state, |s, doc_id| {
        let head = crud::head_revision(s.store.conn(), doc_id)
            .str_err()?
            .ok_or_else(|| "no head revision".to_string())?;
        let outcome = aidoc::merge_branch(&mut s.store, doc_id, &branch, reason)
            .map_err(|e| format!("merge: {e}"))?;
        Ok(MergeResult {
            branch,
            parent: head,
            new_revision: outcome.as_str().to_string(),
        })
    })
}

async fn checkout(
    state: Arc<ServerState>,
    args: serde_json::Map<String, serde_json::Value>,
) -> Result<String, String> {
    let name: String = need(&args, "branch")?;
    with_doc(&state, |s, doc_id| {
        aidoc::checkout_branch(&mut s.store, doc_id, &name).map_err(|e| e.to_string())
    })
}

// ---------- server ----------

#[derive(Clone)]
struct AIDocServer {
    state: Arc<ServerState>,
    tools: Arc<HashMap<String, ToolEntry>>,
}

type ToolHandler = dyn Fn(
        Arc<ServerState>,
        serde_json::Map<String, serde_json::Value>,
    ) -> BoxFuture<'static, Result<serde_json::Value, String>>
    + Send
    + Sync;

struct ToolEntry {
    description: &'static str,
    handler: Box<ToolHandler>,
}

impl AIDocServer {
    fn new() -> Self {
        let state = Arc::new(ServerState::default());
        let mut tools: HashMap<String, ToolEntry> = HashMap::new();

        tools.insert(
            "init_aidoc".into(),
            tool_entry("Create a new .aidoc package and open it.", |s, a| {
                init_aidoc(s, a)
            }),
        );
        tools.insert(
            "open_aidoc".into(),
            tool_entry("Open an existing .aidoc package.", |s, a| open_aidoc(s, a)),
        );
        tools.insert(
            "save_aidoc".into(),
            tool_entry("Persist the current document.", |s, _| save_aidoc(s)),
        );
        tools.insert(
            "list_nodes".into(),
            tool_entry("List every node in the current document.", |s, _| {
                list_nodes(s)
            }),
        );
        tools.insert(
            "search_nodes".into(),
            tool_entry(
                "Case-insensitive substring search over node content. \
                 Returns up to `limit` matches (default 20). Empty query returns []. \
                 Arguments: {query: string, limit?: number}",
                |s, a| search_nodes(s, a),
            ),
        );
        tools.insert(
            "show_node".into(),
            tool_entry("Show one node by id.", |s, a| show_node(s, a)),
        );
        tools.insert(
            "create_node".into(),
            tool_entry("Create a new node with content.", |s, a| create_node(s, a)),
        );
        tools.insert(
            "update_node".into(),
            tool_entry("Update a node's content.", |s, a| update_node(s, a)),
        );
        tools.insert(
            "delete_node".into(),
            tool_entry("Delete a node by id.", |s, a| delete_node(s, a)),
        );
        tools.insert(
            "apply_operation".into(),
            tool_entry("Apply a raw Operation JSON object.", |s, a| {
                apply_operation_tool(s, a)
            }),
        );
        tools.insert(
            "history".into(),
            tool_entry("List revision history.", |s, a| history(s, a)),
        );
        tools.insert(
            "revert".into(),
            tool_entry("Revert to a previous revision.", |s, a| revert_tool(s, a)),
        );
        tools.insert(
            "export_html".into(),
            tool_entry("Render the document as standalone HTML.", |s, _| {
                export_html(s)
            }),
        );
        tools.insert(
            "validate".into(),
            tool_entry("Run all validators.", |s, _| validate(s)),
        );
        tools.insert(
            "branch".into(),
            tool_entry(
                "Tag the current head with a named branch (spec §34).",
                |s, a| branch(s, a),
            ),
        );
        tools.insert(
            "merge".into(),
            tool_entry(
                "Merge a named branch back into main (spec §35).",
                |s, a| merge(s, a),
            ),
        );
        tools.insert(
            "checkout".into(),
            tool_entry("Switch to main or a named branch tip. Arguments: {branch: string}.", |s, a| checkout(s, a)),
        );

        Self {
            state,
            tools: Arc::new(tools),
        }
    }
}

fn tool_entry<F, Fut, T>(description: &'static str, h: F) -> ToolEntry
where
    F: Fn(Arc<ServerState>, serde_json::Map<String, serde_json::Value>) -> Fut
        + Send
        + Sync
        + Clone
        + 'static,
    Fut: std::future::Future<Output = Result<T, String>> + Send + 'static,
    T: Serialize + Send + 'static,
{
    let handler = move |s: Arc<ServerState>,
                        args: serde_json::Map<String, serde_json::Value>|
          -> BoxFuture<'static, Result<serde_json::Value, String>> {
        let h = h.clone();
        let fut = async move {
            match h(s, args).await {
                Ok(v) => serde_json::to_value(v).map_err(|e| format!("serialize: {e}")),
                Err(e) => Err(e),
            }
        };
        Box::pin(fut)
    };
    ToolEntry {
        description,
        handler: Box::new(handler),
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
        let schema = std::sync::Arc::new(serde_json::Map::new());
        let tools = self
            .tools
            .iter()
            .map(|(name, entry)| {
                let mut t = Tool::new(name.clone(), entry.description, schema.clone());
                t.description = Some(entry.description.into());
                t
            })
            .collect();
        Ok(ListToolsResult {
            tools,
            ..Default::default()
        })
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
        let args: serde_json::Map<String, serde_json::Value> =
            request.arguments.unwrap_or_default();
        let state = self.state.clone();
        match (entry.handler)(state, args).await {
            Ok(serde_json::Value::Null) => Ok(CallToolResult::success(vec![Content::text("ok")])),
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

fn opt<T: serde::de::DeserializeOwned>(
    args: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<T> {
    args.get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

async fn run_op(
    state: Arc<ServerState>,
    op_type: OperationType,
    target: String,
    content: Option<String>,
) -> Result<RevisionDto, String> {
    let _doc_id = current_doc_id(&state)?;
    let head = with_doc(&state, |s, doc_id| {
        crud::head_revision(s.store.conn(), doc_id)
            .str_err()?
            .ok_or_else(|| "no head revision".to_string())
    })?;

    let op = Operation {
        id: OpId::new(format!("OP-{}", chrono::Utc::now().timestamp_millis())),
        op_type,
        target: Some(NodeId::from_validated(&target)),
        expected_revision: RevisionId::new(head),
        expected_hash: None,
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
        branch: None,
    }
}

fn seed_root_and_r000(store: &mut Store, doc_id: &str, title: &str) -> anyhow::Result<()> {
    use aidoc::NodeKind;
    use aidoc_storage::AnyhowErr;
    store.tx::<_, _, AnyhowErr>(|tx| {
        let doc = Document::new(
            doc_id.to_string(),
            title.to_string(),
            NodeId::from_validated("root"),
        );
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
            branch: None,
        };
        crud::insert_revision(tx, doc_id, &rev, true)?;
        crud::save_snapshot(tx, doc_id, "R000", &[root])?;
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
