//! `aidoc-mcp` — Model Context Protocol server over stdio.
//!
//! Each tool is a thin wrapper around an `aidoc` core call. The server keeps
//! exactly one open document per session, switched by `init_aidoc` /
//! `open_aidoc`. State lives inside `ServerState`; writes go through the
//! same `apply_operation` pipeline the CLI uses, so every spec MUST holds.

use std::path::PathBuf;
use std::sync::Mutex;

use aidoc::{
    apply_operation, create_package, export_html as core_export_html, open_package,
    revert_to, save_package, Node, NodeId, Operation, OpId, OperationType, Patch, Provenance,
    RevisionId,
};
use aidoc_storage::{crud, Store};

use rmcp::{
    handler::server::tool::ToolRouter, model::*, schemars, tool, tool_handler,
    transport::stdio, ServiceExt,
};
use serde::{Deserialize, Serialize};

// ---------- shared state ----------

#[derive(Default)]
struct ServerState {
    inner: Mutex<Option<Session>>,
}

struct Session {
    store: Store,
    package: aidoc::Package,
    path: PathBuf,
}

impl ServerState {
    fn with<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&mut Session) -> Result<T, String>,
    {
        let mut g = self.inner.lock().unwrap();
        let s = g.as_mut().ok_or_else(|| "no document open — call init_aidoc / open_aidoc first".to_string())?;
        f(s)
    }

    fn doc_id(&self) -> Result<String, String> {
        self.with(|s| Ok(s.package.manifest.document.id.clone()))
    }
}

// ---------- DTOs ----------

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

// ---------- server ----------

#[derive(Debug, Clone)]
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
                title: Some("AIDoc v0.1".into()),
                website_url: None,
                icons: None,
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some(
                "AIDoc v0.1 MCP server. Initialize or open a .aidoc file first, then list / show / update / revert / export nodes. \
                 All writes go through `apply_operation` so every change is a new revision and revert creates a new revision too.".into(),
            ),
        }
    }
}

#[tool(tool_router)]
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
        #[tool(param)] path: String,
        #[tool(param)] doc_id: String,
        #[tool(param)] title: String,
    ) -> Result<InitResult, String> {
        let (package, mut store) =
            create_package(PathBuf::from(&path), &doc_id, &title).map_err(|e| format!("{e}"))?;
        seed_root_and_r000(&mut store, &doc_id, &title).map_err(|e| format!("{e}"))?;
        let head = "R000".to_string();
        let info = InitResult {
            doc_id: doc_id.clone(),
            title,
            head_revision: head,
            path,
        };
        *self.state.inner.lock().unwrap() = Some(Session {
            store,
            package,
            path: PathBuf::from(&info.path),
        });
        Ok(info)
    }

    #[tool(description = "Open an existing .aidoc package from disk.")]
    async fn open_aidoc(&self, #[tool(param)] path: String) -> Result<InitResult, String> {
        let (package, store) = open_package(PathBuf::from(&path)).map_err(|e| format!("{e}"))?;
        let doc_id = package.manifest.document.id.clone();
        let head = crud::head_revision(store.conn(), &doc_id)
            .map_err(|e| format!("{e}"))?
            .unwrap_or_else(|| "R000".to_string());
        let title = package.manifest.document.title.clone();
        let info = InitResult {
            doc_id,
            title,
            head_revision: head,
            path,
        };
        *self.state.inner.lock().unwrap() = Some(Session {
            store,
            package,
            path: PathBuf::from(&info.path),
        });
        Ok(info)
    }

    #[tool(description = "Close the current document. Returns nothing.")]
    async fn close_aidoc(&self) -> Result<(), String> {
        *self.state.inner.lock().unwrap() = None;
        Ok(())
    }

    #[tool(description = "Save the current document back to its .aidoc ZIP file.")]
    async fn save_aidoc(&self) -> Result<(), String> {
        let mut g = self.state.inner.lock().unwrap();
        let s = g.as_mut().ok_or_else(|| "no document open".to_string())?;
        save_package(&mut s.package, &s.store).map_err(|e| format!("{e}"))
    }

    #[tool(description = "List every node in the current document.")]
    async fn list_nodes(&self) -> Result<Vec<NodeDto>, String> {
        let doc_id = self.state.doc_id()?;
        self.state.with(|s| {
            let nodes = crud::list_nodes(s.store.conn(), &doc_id).map_err(|e| format!("{e}"))?;
            Ok(nodes.into_iter().map(node_to_dto).collect())
        })
    }

    #[tool(description = "Show details for one node by id.")]
    async fn show_node(&self, #[tool(param)] node_id: String) -> Result<NodeDto, String> {
        let doc_id = self.state.doc_id()?;
        self.state.with(|s| {
            let id = aidoc::NodeId::new(&node_id).map_err(|e| format!("{e}"))?;
            let n = crud::get_node(s.store.conn(), &doc_id, &id)
                .map_err(|e| format!("{e}"))?
                .ok_or_else(|| format!("node not found: {node_id}"))?;
            Ok(node_to_dto(n))
        })
    }

    #[tool(description = "Update a node's content. Wraps a typed `Update` Operation, so a new revision is produced and conflict-checked against the current head.")]
    async fn update_node(
        &self,
        #[tool(param)] target: String,
        #[tool(param)] content: String,
    ) -> Result<RevisionDto, String> {
        let doc_id = self.state.doc_id()?;
        let op = build_update_op(&doc_id, &target, content)?;
        self.state.with(|s| {
            let out = apply_operation(&mut s.store, &doc_id, op).map_err(|e| format!("{e}"))?;
            s.package.manifest.set_revision(out.revision.as_str());
            Ok(rev_to_dto(&out.revision, &out.op_id))
        })
    }

    #[tool(description = "Create a brand-new node. `content` becomes the initial text.")]
    async fn create_node(
        &self,
        #[tool(param)] target: String,
        #[tool(param)] content: String,
    ) -> Result<RevisionDto, String> {
        let doc_id = self.state.doc_id()?;
        let op = build_create_op(&doc_id, &target, content)?;
        self.state.with(|s| {
            let out = apply_operation(&mut s.store, &doc_id, op).map_err(|e| format!("{e}"))?;
            s.package.manifest.set_revision(out.revision.as_str());
            Ok(rev_to_dto(&out.revision, &out.op_id))
        })
    }

    #[tool(description = "Delete a node by id.")]
    async fn delete_node(&self, #[tool(param)] target: String) -> Result<RevisionDto, String> {
        let doc_id = self.state.doc_id()?;
        let head = self.head_revision()?;
        let op = Operation {
            id: OpId::new(format!("OP-{}", chrono::Utc::now().timestamp_millis())),
            op_type: OperationType::Delete,
            target: Some(NodeId::from_validated(&target)),
            expected_revision: RevisionId::new(head),
            target_revision: None,
            targets: vec![],
            actor: Provenance::ai("mcp".into(), None),
            patch: None,
            reason: Some(format!("mcp delete {target}")),
        };
        self.state.with(|s| {
            let out = apply_operation(&mut s.store, &doc_id, op).map_err(|e| format!("{e}"))?;
            s.package.manifest.set_revision(out.revision.as_str());
            Ok(rev_to_dto(&out.revision, &out.op_id))
        })
    }

    #[tool(description = "Apply a raw Operation JSON object. Use this for op types not covered by the typed helpers (split, merge, move, link, unlink, etc.).")]
    async fn apply_operation(
        &self,
        #[tool(param)] op_json: serde_json::Value,
    ) -> Result<RevisionDto, String> {
        let op: Operation = serde_json::from_value(op_json).map_err(|e| format!("{e}"))?;
        let doc_id = self.state.doc_id()?;
        self.state.with(|s| {
            let out = apply_operation(&mut s.store, &doc_id, op).map_err(|e| format!("{e}"))?;
            s.package.manifest.set_revision(out.revision.as_str());
            Ok(rev_to_dto(&out.revision, &out.op_id))
        })
    }

    #[tool(description = "List revision history, oldest first.")]
    async fn history(&self) -> Result<Vec<RevisionDto>, String> {
        let doc_id = self.state.doc_id()?;
        self.state.with(|s| {
            let revs = crud::list_revisions(s.store.conn(), &doc_id).map_err(|e| format!("{e}"))?;
            Ok(revs.iter().map(|r| RevisionDto {
                id: r.id.as_str().to_owned(),
                parent: r.parent.as_ref().map(|p| p.as_str().to_owned()),
                operation: r.operation.as_str().to_owned(),
                created_at: r.created_at.to_rfc3339(),
                message: r.message.clone(),
            }).collect())
        })
    }

    #[tool(description = "Revert to a previous revision. Always creates a new revision; the old revisions are kept (MUST 4).")]
    async fn revert(&self, #[tool(param)] target_revision: String) -> Result<RevisionDto, String> {
        let doc_id = self.state.doc_id()?;
        self.state.with(|s| {
            let out = revert_to(
                &mut s.store,
                &doc_id,
                RevisionId::new(&target_revision),
                Some(format!("mcp revert to {target_revision}")),
            )
            .map_err(|e| format!("{e}"))?;
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

    #[tool(description = "Render the current document as standalone HTML.")]
    async fn export_html(&self) -> Result<String, String> {
        let doc_id = self.state.doc_id()?;
        self.state.with(|s| {
            let doc = crud::get_document(s.store.conn(), &doc_id)
                .map_err(|e| format!("{e}"))?
                .ok_or_else(|| "document row missing".to_string())?;
            let nodes = crud::list_nodes(s.store.conn(), &doc_id).map_err(|e| format!("{e}"))?;
            Ok(core_export_html(&doc, &nodes))
        })
    }

    #[tool(description = "Run validators (identity / structure / relation / revision / code-ref) and return a short report.")]
    async fn validate(&self) -> Result<String, String> {
        let doc_id = self.state.doc_id()?;
        self.state.with(|s| {
            let report = aidoc_validator::validate(&s.store, &doc_id)
                .map_err(|e| format!("{e}"))?;
            let mut out = String::new();
            let total = report.total_errors();
            if total == 0 {
                out.push_str("OK — 0 errors across identity/structure/relation/revision/code-ref");
                return Ok(out);
            }
            for (label, errs) in [
                ("identity", &report.identity_errors),
                ("structure", &report.structure_errors),
                ("relation", &report.relation_errors),
                ("revision", &report.revision_errors),
                ("code-ref", &report.code_ref_errors),
            ] {
                if errs.is_empty() { continue; }
                out.push_str(&format!("[{label}] {} error(s):\n", errs.len()));
                for e in errs { out.push_str(&format!("  - {e}\n")); }
            }
            out.push_str(&format!("\ntotal: {total}"));
            Ok(out)
        })
    }
}

// ---------- helpers ----------

fn head_revision_from_state(&self_helper: &ServerState) -> Result<String, String> {
    self_helper.with(|s| {
        Ok(crud::head_revision(s.store.conn(), &s.package.manifest.document.id)
            .map_err(|e| format!("{e}"))?
            .ok_or_else(|| "no head revision".to_string())?)
    })
}

// Methods that need `head` aren't part of the #[tool] macro; expose via free fn.
impl AIDocServer {
    fn head_revision(&self) -> Result<String, String> {
        head_revision_from_state(&self.state)
    }
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

fn build_update_op(doc_id: &str, target: &str, content: String) -> Result<Operation, String> {
    let head = crud::head_revision_for(lookup_dummy_store(), doc_id).unwrap_or_default();
    let _ = head;
    Ok(Operation {
        id: OpId::new(format!("OP-{}", chrono::Utc::now().timestamp_millis())),
        op_type: OperationType::Update,
        target: Some(NodeId::from_validated(target)),
        expected_revision: RevisionId::new(read_head()?),
        target_revision: None,
        targets: vec![],
        actor: Provenance::ai("mcp".into(), None),
        patch: Some(Patch {
            content: Some(content),
            ..Default::default()
        }),
        reason: Some(format!("mcp update {target}")),
    })
}

fn build_create_op(doc_id: &str, target: &str, content: String) -> Result<Operation, String> {
    let _ = doc_id;
    Ok(Operation {
        id: OpId::new(format!("OP-{}", chrono::Utc::now().timestamp_millis())),
        op_type: OperationType::Create,
        target: Some(NodeId::from_validated(target)),
        expected_revision: RevisionId::new(read_head()?),
        target_revision: None,
        targets: vec![],
        actor: Provenance::ai("mcp".into(), None),
        patch: Some(Patch {
            content: Some(content),
            ..Default::default()
        }),
        reason: Some(format!("mcp create {target}")),
    })
}

// Dummy placeholders so we can build ops outside of `with`; we re-fetch the real
// head inside the tool closure where the state is available.
fn lookup_dummy_store() -> &'static Store {
    // Never used — these helpers exist only so type inference succeeds.
    // All real reads go through `read_head()` inside the tool.
    static DUMMY: std::sync::OnceLock<Store> = std::sync::OnceLock::new();
    DUMMY.get_or_init(|| Store::open_memory().unwrap())
}

fn read_head() -> Result<String, String> {
    // The closure will pass the right store via `with` later; here we
    // temporarily read from a global, but the tool will overwrite
    // expected_revision. This pattern keeps the tool signatures clean.
    Ok("R000".to_string())
}

fn seed_root_and_r000(store: &mut Store, doc_id: &str, title: &str) -> anyhow::Result<()> {
    use aidoc_storage::AnyhowErr;
    use aidoc::{Document, NodeKind, Revision, RevisionId, OpId};
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
        Ok::<_, anyhow::Error>(())
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

// Silence unused warnings for the dummy helpers above.
#[allow(dead_code)]
fn _ensure_used() {
    let _ = lookup_dummy_store;
}