use anyhow::{Context, Result};

use aidoc::storage::crud;

use crate::session::Session;

/// Compare two revisions by loading the snapshot table for each and walking
/// the node id sets. `to` defaults to the current head when omitted; `from`
/// defaults to the revision immediately before `to`.
///
/// Each revision in the document has a snapshot row (saved by `apply`); we
/// never compare against the live `nodes` table directly so the output
/// reflects what the document *was* at that revision, even after subsequent
/// edits have rolled it forward.
pub fn run(path: &str, from: Option<&str>, to: Option<&str>) -> Result<()> {
    let s = Session::open(path)?;
    let doc_id = s
        .package
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no package open"))?
        .manifest
        .document
        .id
        .clone();

    let head = crud::head_revision(s.store.conn(), &doc_id)?
        .ok_or_else(|| anyhow::anyhow!("no head revision"))?;
    let to_rev = to.unwrap_or(&head).to_string();
    let from_rev = match from {
        Some(f) => f.to_string(),
        None => default_from_rev(&s, &doc_id, &to_rev)?,
    };

    let from_nodes = crud::load_snapshot(s.store.conn(), &doc_id, &from_rev)
        .map_err(|e| anyhow::anyhow!("load snapshot {from_rev}: {e}"))?
        .unwrap_or_default();
    let to_nodes = crud::load_snapshot(s.store.conn(), &doc_id, &to_rev)
        .map_err(|e| anyhow::anyhow!("load snapshot {to_rev}: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("no snapshot for {to_rev}"))?;

    println!("diff {from_rev} -> {to_rev} (head={head})");

    let from_map: std::collections::HashMap<&str, &str> = from_nodes
        .iter()
        .map(|n| (n.id.as_str(), n.content.as_str()))
        .collect();
    let to_map: std::collections::HashMap<&str, &str> = to_nodes
        .iter()
        .map(|n| (n.id.as_str(), n.content.as_str()))
        .collect();

    let mut added = 0usize;
    let mut removed = 0usize;
    let mut changed = 0usize;
    let mut same = 0usize;

    let mut all_ids: Vec<&str> = from_map
        .keys()
        .copied()
        .chain(to_map.keys().copied())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    all_ids.sort_unstable();

    for id in &all_ids {
        match (from_map.get(id), to_map.get(id)) {
            (None, Some(new_content)) => {
                added += 1;
                println!("+ {}  {}", id, preview(new_content));
            }
            (Some(old_content), None) => {
                removed += 1;
                println!("- {}  {}", id, preview(old_content));
            }
            (Some(a), Some(b)) if a != b => {
                changed += 1;
                println!("~ {}  ({}) -> ({})", id, preview(a), preview(b));
            }
            (Some(_), Some(_)) => same += 1,
            (None, None) => unreachable!(),
        }
    }

    println!(
        "summary: +{} added, -{} removed, ~{} changed, ={} unchanged (total ids {})",
        added,
        removed,
        changed,
        same,
        all_ids.len()
    );
    Ok(())
}

fn preview(s: &str) -> String {
    let one_line = s.lines().next().unwrap_or("").trim();
    let clipped: String = one_line.chars().take(80).collect();
    if one_line.chars().count() > 80 {
        format!("{clipped}…")
    } else {
        clipped
    }
}

fn default_from_rev(s: &Session, doc_id: &str, to_rev: &str) -> Result<String> {
    let revs = crud::list_revisions(s.store.conn(), doc_id)
        .context("list revisions")?
        .into_iter()
        .map(|r| r.id.as_str().to_string())
        .collect::<Vec<_>>();
    let pos = revs
        .iter()
        .position(|r| r == to_rev)
        .ok_or_else(|| anyhow::anyhow!("revision {to_rev} not found"))?;
    if pos == 0 {
        Ok(revs[0].clone())
    } else {
        Ok(revs[pos - 1].clone())
    }
}
