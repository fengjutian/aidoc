//! Tauri shell: thin glue between the React UI and the `aidoc` Rust core.
//!
//! All commands return `Result<T, String>` so the frontend gets a clean
//! `invoke().then(...).catch(err => ...)` flow.

use aidoc::{
    ChangeType, Document, Node, NodeId, NodeKind, OpId, Operation, OperationType, Patch,
    Provenance, Revision, RevisionId, apply_operation, create_package, open_package, revert_to,
    save_package,
    validator::{ValidationCategory, validate as core_validate},
};
use aidoc_storage::{Store, crud};

use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

#[derive(Default)]
struct AppState {
    inner: Mutex<Option<SessionHandle>>,
}

struct SessionHandle {
    store: Store,
    package: aidoc::Package,
}

#[derive(Debug, Serialize)]
struct InfoDto {
    doc_id: String,
    title: String,
    head_revision: String,
    entry: String,
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

fn err<E: std::fmt::Display>(s: E) -> String {
    s.to_string()
}

/// Resolve the AI agent script path. Search order:
/// 1. `AIDOC_AI_AGENT` env var (absolute path to agent.py).
/// 2. `<workspace_root>/apps/ai/agent.py` (works when launched from the repo root).
/// 3. `apps/ai/agent.py` next to the current exe (dev-time fallback).
fn ai_agent_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("AIDOC_AI_AGENT") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    // Walk up from the exe to find a workspace containing `apps/ai/agent.py`.
    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent().map(|p| p.to_path_buf());
        while let Some(dir) = cur {
            let candidate = dir.join("apps").join("ai").join("agent.py");
            if candidate.exists() {
                return Some(candidate);
            }
            cur = dir.parent().map(|p| p.to_path_buf());
        }
    }
    None
}

fn ai_chat_impl(
    prompt: String,
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    doc_path: Option<String>,
) -> Result<String, String> {
    let script = ai_agent_path().ok_or_else(|| {
        "Could not locate apps/ai/agent.py. Set the AIDOC_AI_AGENT env var to its absolute path.".to_string()
    })?;
    let key = api_key
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("OPENAI_API_KEY").ok())
        .ok_or_else(|| {
            "OPENAI_API_KEY not set. Add it in Settings → AI (or export the env var)."
                .to_string()
        })?;
    let base = base_url.unwrap_or_else(|| {
        std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".into())
    });
    let mdl = model.unwrap_or_else(|| {
        std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into())
    });

    let mcp_bin = "target\\debug\\aidoc-mcp.exe";
    let mut cmd = Command::new("python");
    cmd.arg(&script)
        .arg("--mcp-bin")
        .arg(mcp_bin)
        .arg("--api-key")
        .arg(&key)
        .arg("--base-url")
        .arg(&base)
        .arg("--model")
        .arg(&mdl)
        .arg("--prompt")
        .arg(&prompt);
    if let Some(p) = doc_path {
        cmd.arg("--doc").arg(p);
    }
    let out = cmd.output().map_err(|e| format!("spawn python: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        return Err(format!(
            "ai_chat exited with status {}: {}",
            out.status,
            stderr.trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[tauri::command]
fn ai_chat(
    prompt: String,
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    doc_path: Option<String>,
) -> Result<String, String> {
    ai_chat_impl(prompt, api_key, base_url, model, doc_path)
}

// ---------------- Commands ----------------

#[tauri::command]
fn init_doc(
    state: tauri::State<'_, AppState>,
    path: String,
    doc_id: String,
    title: String,
) -> Result<InfoDto, String> {
    let (package, store) =
        create_package(PathBuf::from(path.clone()), &doc_id, &title).map_err(err)?;
    let mut store = store;
    seed_root(&mut store, &doc_id, &title).map_err(err)?;
    seed_initial_revision(&mut store, &doc_id).map_err(err)?;
    let info = read_info(&store, &package);
    *state.inner.lock().unwrap() = Some(SessionHandle { store, package });
    Ok(info)
}

#[tauri::command]
fn open_doc(state: tauri::State<'_, AppState>, path: String) -> Result<InfoDto, String> {
    let mut p = PathBuf::from(path);
    let (package, store) = open_package(&mut p).map_err(err)?;
    let info = read_info(&store, &package);
    *state.inner.lock().unwrap() = Some(SessionHandle { store, package });
    Ok(info)
}

#[tauri::command]
fn save_doc(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(|| err("no doc open"))?;
    save_package(&mut s.package, &s.store).map_err(err)
}

#[tauri::command]
fn list_nodes(state: tauri::State<'_, AppState>) -> Result<Vec<NodeDto>, String> {
    let g = state.inner.lock().unwrap();
    let s = g.as_ref().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let nodes = crud::list_nodes(s.store.conn(), &doc_id).map_err(err)?;
    Ok(nodes
        .into_iter()
        .map(|n| NodeDto {
            id: n.id.as_str().to_owned(),
            kind: format!("{:?}", n.kind).to_lowercase(),
            parent: n.parent.as_ref().map(|p| p.as_str().to_owned()),
            position: n.position,
            content: n.content,
        })
        .collect())
}

#[tauri::command]
fn list_revisions(state: tauri::State<'_, AppState>) -> Result<Vec<RevisionDto>, String> {
    let g = state.inner.lock().unwrap();
    let s = g.as_ref().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let revs = crud::list_revisions(s.store.conn(), &doc_id).map_err(err)?;
    Ok(revs
        .into_iter()
        .map(|r| RevisionDto {
            id: r.id.as_str().to_owned(),
            parent: r.parent.as_ref().map(|p| p.as_str().to_owned()),
            operation: r.operation.as_str().to_owned(),
            created_at: r.created_at.to_rfc3339(),
            message: r.message,
        })
        .collect())
}

#[tauri::command]
fn update_node(
    state: tauri::State<'_, AppState>,
    target: String,
    content: String,
) -> Result<String, String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let head = crud::head_revision(s.store.conn(), &doc_id)
        .map_err(err)?
        .ok_or_else(|| err("no head revision"))?;

    let op = Operation {
        id: OpId::new(format!("OP-{}", chrono::Utc::now().timestamp_millis())),
        op_type: OperationType::Update,
        target: Some(NodeId::from_validated(target)),
        expected_revision: RevisionId::new(head),
        expected_hash: None,
        target_revision: None,
        targets: vec![],
        actor: Provenance::human(Some("desktop".into())),
        patch: Some(Patch {
            content: Some(content),
            ..Default::default()
        }),
        reason: Some("UI edit".into()),
    };
    let out = apply_operation(&mut s.store, &doc_id, op).map_err(err)?;
    s.package.manifest.set_revision(out.revision.as_str());
    Ok(out.revision.as_str().to_owned())
}

#[tauri::command]
fn revert(state: tauri::State<'_, AppState>, target: String) -> Result<String, String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let out = revert_to(
        &mut s.store,
        &doc_id,
        RevisionId::new(target),
        Some("UI revert".into()),
    )
    .map_err(err)?;
    s.package.manifest.set_revision(out.new_revision.as_str());
    Ok(out.new_revision.as_str().to_owned())
}

#[derive(Debug, Serialize)]
struct ValidationFindingDto {
    category: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct ValidationReportDto {
    clean: bool,
    total: usize,
    findings: Vec<ValidationFindingDto>,
}

#[tauri::command]
fn validate_aidoc(state: tauri::State<'_, AppState>) -> Result<ValidationReportDto, String> {
    let g = state.inner.lock().unwrap();
    let s = g.as_ref().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let report = core_validate(&s.store, &doc_id).map_err(err)?;
    Ok(ValidationReportDto {
        clean: report.is_clean(),
        total: report.total_errors(),
        findings: report
            .findings
            .into_iter()
            .map(|f| ValidationFindingDto {
                category: match f.category {
                    ValidationCategory::Identity => "identity",
                    ValidationCategory::Structure => "structure",
                    ValidationCategory::Relation => "relation",
                    ValidationCategory::Revision => "revision",
                    ValidationCategory::CodeRef => "code-ref",
                }
                .to_string(),
                message: f.message,
            })
            .collect(),
    })
}

fn build_op(
    store: &Store,
    doc_id: &str,
    op_type: OperationType,
    target: Option<NodeId>,
    patch: Option<Patch>,
    reason: &str,
) -> Result<Operation, String> {
    let head = crud::head_revision(store.conn(), doc_id)
        .map_err(err)?
        .ok_or_else(|| err("no head revision"))?;
    Ok(Operation {
        id: OpId::new(format!("OP-{}", chrono::Utc::now().timestamp_millis())),
        op_type,
        target,
        expected_revision: RevisionId::new(head),
        expected_hash: None,
        target_revision: None,
        targets: vec![],
        actor: Provenance::human(Some("desktop".into())),
        patch,
        reason: Some(reason.into()),
    })
}

#[tauri::command]
fn create_node(
    state: tauri::State<'_, AppState>,
    id: String,
    kind: String,
    content: String,
) -> Result<String, String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let target = NodeId::from_validated(&id);
    let mut patch = Patch::default();
    patch.content = Some(content);
    patch.semantic_type = Some(kind);
    let op = build_op(&s.store, &doc_id, OperationType::Create, Some(target.clone()), Some(patch), "UI create")?;
    let out = apply_operation(&mut s.store, &doc_id, op).map_err(err)?;
    s.package.manifest.set_revision(out.revision.as_str());
    Ok(target.as_str().to_owned())
}

#[tauri::command]
fn delete_node(state: tauri::State<'_, AppState>, target: String) -> Result<String, String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let target_id = NodeId::from_validated(&target);
    let op = build_op(&s.store, &doc_id, OperationType::Delete, Some(target_id), None, "UI delete")?;
    let out = apply_operation(&mut s.store, &doc_id, op).map_err(err)?;
    s.package.manifest.set_revision(out.revision.as_str());
    Ok(out.revision.as_str().to_owned())
}

fn parse_kind(s: &str) -> Result<NodeKind, String> {
    match s {
        "section" => Ok(NodeKind::Section),
        "paragraph" => Ok(NodeKind::Paragraph),
        "heading" => Ok(NodeKind::Heading),
        "list" => Ok(NodeKind::List),
        "code" => Ok(NodeKind::Code),
        "blockquote" => Ok(NodeKind::Blockquote),
        "diagram" => Ok(NodeKind::Diagram),
        "code-ref" => Ok(NodeKind::CodeRef),
        "requirement" => Ok(NodeKind::Requirement),
        "decision" => Ok(NodeKind::Decision),
        "problem" => Ok(NodeKind::Problem),
        "solution" => Ok(NodeKind::Solution),
        "reference" => Ok(NodeKind::Reference),
        "generic" => Ok(NodeKind::Generic),
        other => Err(format!("unknown kind: {other}")),
    }
}

#[tauri::command]
fn set_node_kind(
    state: tauri::State<'_, AppState>,
    target: String,
    kind: String,
) -> Result<String, String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let target_id = NodeId::from_validated(&target);
    let node_kind = parse_kind(&kind)?;
    let mut patch = Patch::default();
    patch.kind = Some(node_kind);
    let op = build_op(
        &s.store,
        &doc_id,
        OperationType::Update,
        Some(target_id),
        Some(patch),
        "UI change kind",
    )?;
    let out = apply_operation(&mut s.store, &doc_id, op).map_err(err)?;
    s.package.manifest.set_revision(out.revision.as_str());
    Ok(out.revision.as_str().to_owned())
}

#[tauri::command]
fn move_node(
    state: tauri::State<'_, AppState>,
    target: String,
    new_position: u32,
) -> Result<String, String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let target_id = NodeId::from_validated(&target);
    let mut patch = Patch::default();
    patch.position = Some(new_position);
    let op = build_op(
        &s.store,
        &doc_id,
        OperationType::Move,
        Some(target_id),
        Some(patch),
        "UI reorder",
    )?;
    let out = apply_operation(&mut s.store, &doc_id, op).map_err(err)?;
    s.package.manifest.set_revision(out.revision.as_str());
    Ok(out.revision.as_str().to_owned())
}

#[derive(Debug, Serialize)]
struct ChangeDto {
    node: String,
    change_type: String,
    summary: Option<String>,
    before_hash: Option<String>,
    after_hash: Option<String>,
}

#[tauri::command]
fn list_changes(state: tauri::State<'_, AppState>, rev_id: String) -> Result<Vec<ChangeDto>, String> {
    let g = state.inner.lock().unwrap();
    let s = g.as_ref().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let changes = crud::list_changes_for_revision(s.store.conn(), &doc_id, &rev_id).map_err(err)?;
    Ok(changes
        .into_iter()
        .map(|c| ChangeDto {
            node: c.node.as_str().to_owned(),
            change_type: match c.change_type {
                ChangeType::ContentUpdate => "content-update",
                ChangeType::Create => "create",
                ChangeType::Delete => "delete",
                ChangeType::Move => "move",
                ChangeType::Rename => "rename",
                ChangeType::Split => "split",
                ChangeType::Merge => "merge",
                ChangeType::Revert => "revert",
                ChangeType::RelationAdd => "relation-add",
                ChangeType::RelationRemove => "relation-remove",
            }
            .to_string(),
            summary: c.summary,
            before_hash: c.before.map(|h| h.hash),
            after_hash: c.after.map(|h| h.hash),
        })
        .collect())
}

#[tauri::command]
fn export_html(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let g = state.inner.lock().unwrap();
    let s = g.as_ref().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let doc = crud::get_document(s.store.conn(), &doc_id)
        .map_err(err)?
        .ok_or_else(|| err("doc missing"))?;
    let nodes = crud::list_nodes(s.store.conn(), &doc_id).map_err(err)?;
    let branch = crud::head_branch(s.store.conn(), &doc_id).map_err(err)?;
    Ok(aidoc::exporter::export_html(
        &doc,
        &nodes,
        branch.as_deref(),
    ))
}

// ---------------- helpers ----------------

fn read_info(store: &Store, package: &aidoc::Package) -> InfoDto {
    let head = crud::head_revision(store.conn(), &package.manifest.document.id)
        .ok()
        .flatten()
        .unwrap_or_else(|| "R000".to_string());
    InfoDto {
        doc_id: package.manifest.document.id.clone(),
        title: package.manifest.document.title.clone(),
        head_revision: head,
        entry: package.manifest.entry.clone(),
    }
}

fn seed_root(store: &mut Store, doc_id: &str, title: &str) -> Result<(), String> {
    use aidoc_storage::AnyhowErr;
    store
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
            Ok::<_, AnyhowErr>(())
        })
        .map_err(|e| err(format!("seed: {e}")))
}

fn seed_initial_revision(store: &mut Store, doc_id: &str) -> Result<(), String> {
    use aidoc_storage::AnyhowErr;
    let rev = Revision {
        id: RevisionId::new("R000"),
        parent: None,
        operation: OpId::new("OP-000"),
        created_at: chrono::Utc::now(),
        message: Some("seed".into()),
        branch: None,
    };
    store
        .tx::<_, _, AnyhowErr>(|tx| {
            crud::insert_revision(tx, doc_id, &rev, true)?;
            Ok::<_, AnyhowErr>(())
        })
        .map_err(|e| err(format!("seed revision: {e}")))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            init_doc,
            open_doc,
            save_doc,
            list_nodes,
            list_revisions,
            update_node,
            revert,
            export_html,
            validate_aidoc,
            create_node,
            delete_node,
            list_changes,
            set_node_kind,
            move_node,
            ai_chat,
        ])
        .run(tauri::generate_context!())
        .expect("error while running AIDoc desktop");
}
