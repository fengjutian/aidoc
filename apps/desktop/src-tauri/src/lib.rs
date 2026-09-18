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
use std::collections::HashMap;
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

fn ai_chat_impl(
    prompt: String,
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    doc_path: Option<String>,
    history: Option<Vec<serde_json::Value>>,
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
    history: Option<Vec<serde_json::Value>>,
) -> Result<String, String> {
    ai_chat_impl(prompt, api_key, base_url, model, doc_path, history)
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running AIDoc desktop");
}
