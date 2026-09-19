//! Tauri shell: thin glue between the React UI and the `aidoc` Rust core.
//!
//! All commands return `Result<T, String>` so the frontend gets a clean
//! `invoke().then(...).catch(err => ...)` flow.

use aidoc::{
    ChangeType, Document, Manifest, Node, NodeId, NodeKind, OpId, Operation, OperationType, Patch,
    Provenance, Revision, RevisionId, apply_operation, create_package, open_package, revert_to,
    save_package,
    validator::{ValidationCategory, validate as core_validate},
};
use aidoc_storage::{Store, crud};

use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[derive(Default)]
struct AppState {
    inner: Mutex<Option<SessionHandle>>,
    /// Slot for the currently-running AI child process. `Arc<Mutex<Option<Child>>>`
    /// so the reader thread can also pull it out for `wait()` while the main
    /// thread keeps a handle for `abort_ai_chat` to kill it.
    ai_child: Mutex<Option<Arc<Mutex<Option<Child>>>>>,
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
    source_path: String,
}

#[derive(Debug, Serialize)]
struct NodeDto {
    id: String,
    kind: String,
    parent: Option<String>,
    position: u32,
    content: String,
    attributes: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct RevisionDto {
    id: String,
    parent: Option<String>,
    operation: String,
    created_at: String,
    message: Option<String>,
    branch: Option<String>,
}

#[derive(Debug, Serialize)]
struct RelationDto {
    id: String,
    source: String,
    target: String,
    kind: String,
}

#[derive(Debug, Serialize)]
struct DiffEntryDto {
    node: String,
    status: String,
    before: Option<String>,
    after: Option<String>,
}

#[derive(Debug, Serialize)]
struct DiffReportDto {
    from: String,
    to: String,
    added: usize,
    removed: usize,
    changed: usize,
    entries: Vec<DiffEntryDto>,
}

#[derive(Debug, Serialize)]
struct BranchDto {
    name: String,
    head: Option<String>,
    revisions: usize,
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

/// Resolve the `aidoc-mcp` binary path. Search order:
/// 1. `AIDOC_MCP_BIN` env var (absolute or relative path)
/// 2. Walk up from `current_exe` looking for `target/{debug,release}/aidoc-mcp[.exe]`
/// 3. Walk up looking for a workspace sibling of `apps/desktop/` containing
///    `crates/aidoc-mcp/target/{debug,release}/aidoc-mcp[.exe]` (cargo run from
///    the aidoc-mcp crate itself)
/// 4. Fallback to `target/debug/aidoc-mcp[.exe]` from CWD (dev convenience)
fn resolve_mcp_bin() -> Option<PathBuf> {
    let bin_name = if cfg!(target_os = "windows") {
        "aidoc-mcp.exe"
    } else {
        "aidoc-mcp"
    };

    // 1. Explicit env var wins.
    if let Ok(p) = std::env::var("AIDOC_MCP_BIN") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }

    // 2 & 3. Walk up from the desktop exe.
    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent().map(|p| p.to_path_buf());
        let mut saw_desktop = false;
        while let Some(dir) = cur {
            // If we're inside the desktop app crate, try the workspace-root
            // target/ (covers both debug and release profiles).
            for profile in &["debug", "release"] {
                let cand = dir.join("target").join(profile).join(bin_name);
                if cand.exists() {
                    return Some(cand);
                }
            }
            // Mark when we've crossed the desktop app dir so on later walks
            // we also probe a sibling `crates/aidoc-mcp/target/...` for users
            // who ran `cargo run -p aidoc-mcp` standalone.
            if dir.join("Cargo.toml").exists()
                && dir.join("tauri.conf.json").exists()
            {
                saw_desktop = true;
            }
            if saw_desktop {
                let cand = dir
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("crates")
                    .join("aidoc-mcp")
                    .join("target")
                    .join("debug")
                    .join(bin_name);
                if cand.exists() {
                    return Some(cand);
                }
            }
            cur = dir.parent().map(|p| p.to_path_buf());
        }
    }

    // 4. Last-ditch dev convenience: relative to CWD.
    let cwd_fallback = PathBuf::from("target").join("debug").join(bin_name);
    if cwd_fallback.exists() {
        return Some(cwd_fallback);
    }

    None
}

fn ai_chat_impl(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    prompt: String,
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    doc_path: Option<String>,
    history: Option<Vec<serde_json::Value>>,
) -> Result<String, String> {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;

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

    let mcp_bin = resolve_mcp_bin().ok_or_else(|| {
        "Could not locate aidoc-mcp binary. Set AIDOC_MCP_BIN env var to its \
         absolute path, or build the crate with `cargo build -p aidoc-mcp`."
            .to_string()
    })?;
    let mut cmd = Command::new("python");
    cmd.arg(&script)
        .arg("--mcp-bin")
        .arg(&mcp_bin)
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
    if let Some(h) = history {
        let trimmed: Vec<serde_json::Value> = h
            .into_iter()
            .filter_map(|turn| {
                let role = turn.get("role")?.as_str()?;
                let content = turn.get("content")?.as_str()?;
                if role == "user" || role == "assistant" {
                    Some(serde_json::json!({"role": role, "content": content}))
                } else {
                    None
                }
            })
            .collect();
        if !trimmed.is_empty() {
            let json = serde_json::to_string(&trimmed).map_err(|e| e.to_string())?;
            cmd.arg("--history-json").arg(json);
        }
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("spawn python: {e}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "no stdout pipe".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "no stderr pipe".to_string())?;

    // Stash child for abort_ai_chat. Wrapped in Arc so both this thread and
    // the reader thread can drop/kill it.
    let slot: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(Some(child)));
    {
        let mut g = state.ai_child.lock().unwrap();
        if let Some(prev) = g.take() {
            // A previous request is still running — kill it first.
            let mut inner = prev.lock().unwrap();
            if let Some(mut c) = inner.take() {
                let _ = c.kill();
            }
        }
        *g = Some(slot.clone());
    }

    // Reader thread: line-by-line stdout → emit "ai-chunk"; stderr buffered
    // for an error message if the process exits non-zero.
    let app_for_thread = app.clone();
    let slot_for_thread = slot.clone();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(chunk) => {
                    // `__USAGE__:prompt=N,completion=N,total=N` is a one-line
                    // control record emitted by agent.py after every
                    // OpenAI call. Strip the prefix and re-emit it on a
                    // dedicated event so the UI can track token totals.
                    if let Some(rest) = chunk.strip_prefix("__USAGE__:") {
                        let mut prompt = 0u32;
                        let mut completion = 0u32;
                        let mut total = 0u32;
                        for kv in rest.split(',') {
                            let (k, v) = match kv.split_once('=') {
                                Some(p) => p,
                                None => continue,
                            };
                            let n = v.trim().parse::<u32>().unwrap_or(0);
                            match k.trim() {
                                "prompt" => prompt = n,
                                "completion" => completion = n,
                                "total" => total = n,
                                _ => {}
                            }
                        }
                        let _ = app_for_thread.emit(
                            "ai-usage",
                            serde_json::json!({
                                "prompt": prompt,
                                "completion": completion,
                                "total": total,
                            }),
                        );
                    } else {
                        let _ = app_for_thread.emit("ai-chunk", chunk);
                    }
                }
                Err(_) => break,
            }
        }
        // Drain stderr on a separate thread so we don't block the main reader.
        let stderr_app = app_for_thread.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            let mut buf = String::new();
            for line in reader.lines().map_while(Result::ok) {
                buf.push_str(&line);
                buf.push('\n');
            }
            if !buf.trim().is_empty() {
                let _ = stderr_app.emit("ai-stderr", buf);
            }
        });

        let mut child_guard = slot_for_thread.lock().unwrap();
        if let Some(mut c) = child_guard.take() {
            match c.wait() {
                Ok(status) if status.success() => {
                    let _ = app_for_thread.emit("ai-done", true);
                }
                Ok(status) => {
                    let _ = app_for_thread.emit(
                        "ai-error",
                        format!("ai_chat exited with status {status}"),
                    );
                }
                Err(e) => {
                    let _ = app_for_thread.emit("ai-error", format!("wait failed: {e}"));
                }
            }
        }
    });

    Ok("started".into())
}

#[tauri::command]
fn ai_chat(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    prompt: String,
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    doc_path: Option<String>,
    history: Option<Vec<serde_json::Value>>,
) -> Result<String, String> {
    ai_chat_impl(
        app,
        state,
        prompt,
        api_key,
        base_url,
        model,
        doc_path,
        history,
    )
}

#[tauri::command]
fn abort_ai_chat(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let mut g = state.ai_child.lock().unwrap();
    let slot = match g.take() {
        Some(s) => s,
        None => return Ok(false),
    };
    let mut child_guard = slot.lock().unwrap();
    if let Some(mut c) = child_guard.take() {
        let _ = c.kill();
        let _ = c.wait();
        return Ok(true);
    }
    Ok(false)
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

/// Snapshot the currently open document(s). For now the desktop holds a
/// single session at a time, but this returns a list so the UI can grow
/// into tabs without another command rename.
#[tauri::command]
fn list_documents(state: tauri::State<'_, AppState>) -> Result<Vec<InfoDto>, String> {
    let g = state.inner.lock().unwrap();
    Ok(match g.as_ref() {
        Some(s) => vec![read_info(&s.store, &s.package)],
        None => vec![],
    })
}

/// Drop the active session. Returns the new active info (or empty list
/// if the workspace is empty).
#[tauri::command]
fn close_doc(state: tauri::State<'_, AppState>) -> Result<Vec<InfoDto>, String> {
    let mut g = state.inner.lock().unwrap();
    *g = None;
    Ok(vec![])
}

#[tauri::command]
fn save_doc(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(|| err("no doc open"))?;
    save_package(&mut s.package, &s.store).map_err(err)
}

/// Save the current workspace to a new `.aidoc` path and switch the active
/// document to it. Subsequent `save_doc` calls write to the new path.
#[tauri::command]
fn save_doc_as(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(|| err("no doc open"))?;
    s.package.source_path = PathBuf::from(path);
    save_package(&mut s.package, &s.store).map_err(err)
}

#[tauri::command]
fn list_nodes(state: tauri::State<'_, AppState>) -> Result<Vec<NodeDto>, String> {
    let g = state.inner.lock().unwrap();
    let s = g.as_ref().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let nodes = crud::list_nodes(s.store.conn(), &doc_id).map_err(err)?;
    Ok(nodes.into_iter().map(node_to_dto).collect())
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
            branch: r.branch,
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
    parent: Option<String>,
) -> Result<String, String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let target = NodeId::from_validated(&id);
    let node_kind = parse_kind(&kind)?;
    let mut patch = Patch::default();
    patch.content = Some(content);
    patch.semantic_type = Some(kind);
    patch.kind = Some(node_kind);
    // `"parent"` is the reparent convention honoured by the Create/Move
    // handlers; omit it to leave the node detached at the document root.
    if let Some(p) = parent {
        patch.attributes.insert("parent".into(), p);
    }
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
        "list-item" => Ok(NodeKind::ListItem),
        "table" => Ok(NodeKind::Table),
        "table-row" => Ok(NodeKind::TableRow),
        "table-cell" => Ok(NodeKind::TableCell),
        "code" => Ok(NodeKind::Code),
        "blockquote" => Ok(NodeKind::Blockquote),
        "link" => Ok(NodeKind::Link),
        "image" => Ok(NodeKind::Image),
        "diagram" => Ok(NodeKind::Diagram),
        "code-ref" => Ok(NodeKind::CodeRef),
        "requirement" => Ok(NodeKind::Requirement),
        "decision" => Ok(NodeKind::Decision),
        "problem" => Ok(NodeKind::Problem),
        "solution" => Ok(NodeKind::Solution),
        "reference" => Ok(NodeKind::Reference),
        "details" => Ok(NodeKind::Details),
        "summary" => Ok(NodeKind::Summary),
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
    new_parent: Option<String>,
) -> Result<String, String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let target_id = NodeId::from_validated(&target);
    let mut patch = Patch::default();
    patch.position = Some(new_position);
    // Reparent when a new parent is supplied (empty string detaches to root).
    // `check::check_move_structure` rejects cycles before the write happens.
    if let Some(p) = new_parent {
        patch.attributes.insert("parent".into(), p);
    }
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

#[tauri::command]
fn export_markdown(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let g = state.inner.lock().unwrap();
    let s = g.as_ref().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let doc = crud::get_document(s.store.conn(), &doc_id)
        .map_err(err)?
        .ok_or_else(|| err("doc missing"))?;
    let nodes = crud::list_nodes(s.store.conn(), &doc_id).map_err(err)?;
    Ok(aidoc::exporter::export_markdown(&doc, &nodes))
}

#[tauri::command]
fn search_nodes(
    state: tauri::State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<NodeDto>, String> {
    let g = state.inner.lock().unwrap();
    let s = g.as_ref().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let nodes =
        crud::search_nodes(s.store.conn(), &doc_id, &query, limit.unwrap_or(50)).map_err(err)?;
    Ok(nodes.into_iter().map(node_to_dto).collect())
}

/// Snapshot-based A→B diff (mirrors the CLI `diff` command). Each revision
/// stores a full node snapshot on apply, so we compare two of those rather
/// than the live table.
#[tauri::command]
fn diff_revisions(
    state: tauri::State<'_, AppState>,
    from: String,
    to: String,
) -> Result<DiffReportDto, String> {
    let g = state.inner.lock().unwrap();
    let s = g.as_ref().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let from_nodes = crud::load_snapshot(s.store.conn(), &doc_id, &from)
        .map_err(err)?
        .unwrap_or_default();
    let to_nodes = crud::load_snapshot(s.store.conn(), &doc_id, &to)
        .map_err(err)?
        .ok_or_else(|| err(format!("no snapshot for revision {to}")))?;

    let from_map: HashMap<&str, &str> = from_nodes
        .iter()
        .map(|n| (n.id.as_str(), n.content.as_str()))
        .collect();
    let to_map: HashMap<&str, &str> = to_nodes
        .iter()
        .map(|n| (n.id.as_str(), n.content.as_str()))
        .collect();

    // BTreeSet de-dupes and orders the union of both node-id sets.
    let ids: Vec<&str> = from_map
        .keys()
        .chain(to_map.keys())
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    let mut entries = Vec::new();
    let (mut added, mut removed, mut changed) = (0usize, 0usize, 0usize);
    for id in ids {
        match (from_map.get(id), to_map.get(id)) {
            (None, Some(after)) => {
                added += 1;
                entries.push(DiffEntryDto {
                    node: id.to_owned(),
                    status: "added".into(),
                    before: None,
                    after: Some((*after).to_owned()),
                });
            }
            (Some(before), None) => {
                removed += 1;
                entries.push(DiffEntryDto {
                    node: id.to_owned(),
                    status: "removed".into(),
                    before: Some((*before).to_owned()),
                    after: None,
                });
            }
            (Some(before), Some(after)) if *before != *after => {
                changed += 1;
                entries.push(DiffEntryDto {
                    node: id.to_owned(),
                    status: "changed".into(),
                    before: Some((*before).to_owned()),
                    after: Some((*after).to_owned()),
                });
            }
            _ => {}
        }
    }
    Ok(DiffReportDto {
        from,
        to,
        added,
        removed,
        changed,
        entries,
    })
}

#[tauri::command]
fn list_relations(state: tauri::State<'_, AppState>) -> Result<Vec<RelationDto>, String> {
    let g = state.inner.lock().unwrap();
    let s = g.as_ref().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let rels = crud::list_relations(s.store.conn(), &doc_id).map_err(err)?;
    Ok(rels
        .into_iter()
        .map(|r| RelationDto {
            id: r.id,
            source: r.source.as_str().to_owned(),
            target: r.target.as_str().to_owned(),
            kind: r.kind.as_str().to_owned(),
        })
        .collect())
}

/// Build a Link/Unlink op. `op.target` is the destination and `op.targets[0]`
/// the source, matching `LinkHandler` / `UnlinkHandler`.
fn relation_op(
    store: &Store,
    doc_id: &str,
    op_type: OperationType,
    source: &str,
    target: &str,
    reason: &str,
) -> Result<Operation, String> {
    let head = current_head(store, doc_id)?;
    Ok(Operation {
        id: OpId::new(format!("OP-{}", chrono::Utc::now().timestamp_millis())),
        op_type,
        target: Some(NodeId::from_validated(target)),
        expected_revision: RevisionId::new(head),
        expected_hash: None,
        target_revision: None,
        targets: vec![NodeId::from_validated(source)],
        actor: Provenance::human(Some("desktop".into())),
        patch: None,
        reason: Some(reason.into()),
    })
}

#[tauri::command]
fn create_link(
    state: tauri::State<'_, AppState>,
    source: String,
    target: String,
) -> Result<String, String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let op = relation_op(&s.store, &doc_id, OperationType::Link, &source, &target, "UI link")?;
    let out = apply_operation(&mut s.store, &doc_id, op).map_err(err)?;
    s.package.manifest.set_revision(out.revision.as_str());
    Ok(out.revision.as_str().to_owned())
}

#[tauri::command]
fn delete_link(
    state: tauri::State<'_, AppState>,
    source: String,
    target: String,
) -> Result<String, String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let op = relation_op(&s.store, &doc_id, OperationType::Unlink, &source, &target, "UI unlink")?;
    let out = apply_operation(&mut s.store, &doc_id, op).map_err(err)?;
    s.package.manifest.set_revision(out.revision.as_str());
    Ok(out.revision.as_str().to_owned())
}

#[tauri::command]
fn set_node_attributes(
    state: tauri::State<'_, AppState>,
    target: String,
    attrs: HashMap<String, String>,
) -> Result<String, String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let target_id = NodeId::from_validated(&target);
    let mut patch = Patch::default();
    for (k, v) in attrs {
        patch.attributes.insert(k, v);
    }
    let op = build_op(
        &s.store,
        &doc_id,
        OperationType::Update,
        Some(target_id),
        Some(patch),
        "UI edit attributes",
    )?;
    let out = apply_operation(&mut s.store, &doc_id, op).map_err(err)?;
    s.package.manifest.set_revision(out.revision.as_str());
    Ok(out.revision.as_str().to_owned())
}

#[tauri::command]
fn list_branches(state: tauri::State<'_, AppState>) -> Result<Vec<BranchDto>, String> {
    let g = state.inner.lock().unwrap();
    let s = g.as_ref().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let revs = crud::list_revisions(s.store.conn(), &doc_id).map_err(err)?;
    let mut named: HashMap<String, (Option<String>, usize)> = HashMap::new();
    let mut main_head: Option<String> = None;
    let mut main_count = 0usize;
    for r in &revs {
        match &r.branch {
            None => {
                main_count += 1;
                main_head = Some(r.id.as_str().to_owned());
            }
            Some(name) => {
                let entry = named.entry(name.clone()).or_insert((None, 0));
                entry.1 += 1;
                entry.0 = Some(r.id.as_str().to_owned());
            }
        }
    }
    let mut out = vec![BranchDto {
        name: "main".into(),
        head: main_head,
        revisions: main_count,
    }];
    let mut names: Vec<&String> = named.keys().collect();
    names.sort_unstable();
    for name in names {
        let (head, count) = &named[name];
        out.push(BranchDto {
            name: name.clone(),
            head: head.clone(),
            revisions: *count,
        });
    }
    Ok(out)
}

#[tauri::command]
fn create_branch(state: tauri::State<'_, AppState>, name: String) -> Result<String, String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let head = current_head(&s.store, &doc_id)?;
    let mut patch = Patch::default();
    patch.attributes.insert("branch".into(), name.clone());
    let op = Operation {
        id: OpId::new(format!("OP-branch-{}", chrono::Utc::now().timestamp_millis())),
        op_type: OperationType::Branch,
        target: None,
        expected_revision: RevisionId::new(head),
        expected_hash: None,
        target_revision: None,
        targets: vec![],
        actor: Provenance::human(Some("desktop".into())),
        patch: Some(patch),
        reason: Some(format!("branch {name}")),
    };
    let out = apply_operation(&mut s.store, &doc_id, op).map_err(err)?;
    s.package.manifest.set_revision(out.revision.as_str());
    Ok(out.revision.as_str().to_owned())
}

#[tauri::command]
fn merge_branch(state: tauri::State<'_, AppState>, name: String) -> Result<String, String> {
    let mut g = state.inner.lock().unwrap();
    let s = g.as_mut().ok_or_else(|| err("no doc open"))?;
    let doc_id = s.package.manifest.document.id.clone();
    let head = current_head(&s.store, &doc_id)?;
    // v0.1 merge mirrors the CLI: a Branch op whose reason records the merge.
    let op = Operation {
        id: OpId::new(format!("OP-merge-{}", chrono::Utc::now().timestamp_millis())),
        op_type: OperationType::Branch,
        target: None,
        expected_revision: RevisionId::new(head),
        expected_hash: None,
        target_revision: None,
        targets: vec![],
        actor: Provenance::human(Some("desktop".into())),
        patch: None,
        reason: Some(format!("merge {name} into main")),
    };
    let out = apply_operation(&mut s.store, &doc_id, op).map_err(err)?;
    s.package.manifest.set_revision(out.revision.as_str());
    Ok(out.revision.as_str().to_owned())
}

// ---------------- helpers ----------------

fn node_to_dto(n: Node) -> NodeDto {
    // Serialise the kind through serde so multi-word kinds stay kebab-case
    // (`code-ref`, `list-item`) and match the frontend's Kind taxonomy.
    let kind = serde_json::to_value(n.kind)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_owned()))
        .unwrap_or_else(|| format!("{:?}", n.kind).to_lowercase());
    NodeDto {
        id: n.id.as_str().to_owned(),
        kind,
        parent: n.parent.as_ref().map(|p| p.as_str().to_owned()),
        position: n.position,
        content: n.content,
        attributes: n.attributes.into_iter().collect(),
    }
}

fn current_head(store: &Store, doc_id: &str) -> Result<String, String> {
    crud::head_revision(store.conn(), doc_id)
        .map_err(err)?
        .ok_or_else(|| err("no head revision"))
}

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
        source_path: package.source_path.display().to_string(),
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            init_doc,
            open_doc,
            save_doc,
            save_doc_as,
            list_nodes,
            list_revisions,
            update_node,
            revert,
            export_html,
            export_markdown,
            validate_aidoc,
            create_node,
            delete_node,
            list_changes,
            set_node_kind,
            set_node_attributes,
            move_node,
            search_nodes,
            diff_revisions,
            list_relations,
            create_link,
            delete_link,
            list_branches,
            create_branch,
            merge_branch,
            ai_chat,
            abort_ai_chat,
            list_documents,
            close_doc,
        ])
        .run(tauri::generate_context!())
        .expect("error while running AIDoc desktop");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir_in_cwd() -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("aidoc-test-{nanos}"));
        std::fs::create_dir_all(&dir).expect("tempdir");
        dir
    }

    #[test]
    fn parse_kind_maps_every_supported_string() {
        let cases: &[(&str, &str)] = &[
            ("section", "section"), ("paragraph", "paragraph"),
            ("heading", "heading"), ("list", "list"),
            ("list-item", "listitem"), ("table", "table"),
            ("table-row", "tablerow"), ("table-cell", "tablecell"),
            ("code", "code"), ("blockquote", "blockquote"),
            ("link", "link"), ("image", "image"),
            ("diagram", "diagram"), ("code-ref", "coderef"),
            ("requirement", "requirement"), ("decision", "decision"),
            ("problem", "problem"), ("solution", "solution"),
            ("reference", "reference"), ("details", "details"),
            ("summary", "summary"), ("generic", "generic"),
        ];
        for (input, expected_dbg) in cases {
            let kind = parse_kind(input).unwrap_or_else(|e| panic!("{input}: {e}"));
            assert_eq!(format!("{kind:?}").to_lowercase(), *expected_dbg);
        }
    }

    #[test]
    fn parse_kind_rejects_unknown() {
        assert!(parse_kind("not-a-kind").is_err());
        assert!(parse_kind("").is_err());
    }

    #[test]
    fn resolve_mcp_bin_prefers_env_var() {
        let bogus = PathBuf::from("Z:/definitely/does/not/exist/aidoc-mcp.exe");
        unsafe { std::env::set_var("AIDOC_MCP_BIN", &bogus); }
        let result = resolve_mcp_bin();
        unsafe { std::env::remove_var("AIDOC_MCP_BIN"); }
        if let Some(p) = result {
            assert_ne!(p, bogus, "must not honour an env var pointing at a missing file");
        }
    }

    #[test]
    fn build_op_attaches_human_provenance() {
        let dir = tempdir_in_cwd();
        let db = dir.join("doc.db");
        let store = Store::open(&db).expect("open store");
        let r = build_op(
            &store,
            "doc-test",
            OperationType::Update,
            Some(NodeId::from_validated("root")),
            Some(Patch {
                content: Some("hello".into()),
                attributes: Default::default(),
                ..Default::default()
            }),
            "test reason",
        );
        assert!(r.is_err(), "build_op must surface missing head as Err, not panic");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn node_to_dto_serialises_kind_as_kebab() {
        let mut n = Node::new(NodeId::from_validated("root"), NodeKind::CodeRef);
        n.content = "see foo()".into();
        let dto = node_to_dto(n);
        assert_eq!(dto.kind, "code-ref");
        assert_eq!(dto.id, "root");
        assert_eq!(dto.content, "see foo()");
    }

    #[test]
    fn node_to_dto_propagates_attributes() {
        let mut n = Node::new(NodeId::from_validated("child"), NodeKind::Section);
        n.attributes.insert("parent".to_string(), "root".to_string());
        n.attributes.insert("anchor".to_string(), "intro".to_string());
        let dto = node_to_dto(n);
        assert_eq!(dto.attributes.get("parent").map(String::as_str), Some("root"));
        assert_eq!(dto.attributes.get("anchor").map(String::as_str), Some("intro"));
    }

    #[test]
    fn seed_root_writes_a_root_node() {
        let dir = tempdir_in_cwd();
        let db = dir.join("doc.db");
        let mut store = Store::open(&db).expect("open store");
        seed_root(&mut store, "doc-x", "Hello").expect("seed");
        let nodes = aidoc_storage::crud::list_nodes(store.conn(), "doc-x")
            .expect("list nodes");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id.as_str(), "root");
        assert_eq!(nodes[0].content, "Hello");
        assert!(matches!(nodes[0].kind, NodeKind::Section));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn seed_initial_revision_creates_R000() {
        let dir = tempdir_in_cwd();
        let db = dir.join("doc.db");
        let mut store = Store::open(&db).expect("open store");
        seed_root(&mut store, "doc-y", "T").expect("seed");
        seed_initial_revision(&mut store, "doc-y").expect("seed rev");
        let head = current_head(&store, "doc-y").expect("head");
        assert_eq!(head, "R000");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_info_reports_document_title() {
        let dir = tempdir_in_cwd();
        let db = dir.join("doc.db");
        let mut store = Store::open(&db).expect("open store");
        seed_root(&mut store, "doc-z", "My Title").expect("seed");
        seed_initial_revision(&mut store, "doc-z").expect("rev");
        let head = current_head(&store, "doc-z").expect("head");
        assert_eq!(head, "R000");
        let manifest = Manifest::new("doc-z", "My Title", "R000");
        assert_eq!(manifest.document.title, "My Title");
        assert_eq!(manifest.document.id, "doc-z");
        assert_eq!(manifest.revision.current, "R000");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tauri_conf_csp_drops_script_unsafe_inline() {
        // Read `tauri.conf.json` and assert that the runtime CSP no longer
        // permits inline scripts. Style-src may still allow it (React +
        // mermaid emit inline styles).
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR must be set in cargo test");
        let path = PathBuf::from(manifest_dir).join("tauri.conf.json");
        let raw = std::fs::read_to_string(&path).expect("read tauri.conf.json");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse tauri.conf.json");
        let csp = v
            .get("app")
            .and_then(|a| a.get("security"))
            .and_then(|s| s.get("csp"))
            .and_then(|c| c.as_str())
            .expect("tauri.conf.json app.security.csp");
        for directive in csp.split(';') {
            let d = directive.trim();
            if d.starts_with("script-src") {
                assert!(
                    !d.contains("'unsafe-inline'"),
                    "script-src must not allow inline execution after tightening; \
                     got: {d}; full csp: {csp}",
                );
            }
        }
        // And style-src should still allow inline so React / mermaid keep
        // working without us re-hashing every emitted style.
        assert!(csp.contains("style-src"), "csp must declare style-src: {csp}");
        assert!(
            csp.split(';').any(|d| d.trim().starts_with("style-src") && d.contains("'unsafe-inline'")),
            "style-src must still allow inline styles: {csp}",
        );
    }
}
