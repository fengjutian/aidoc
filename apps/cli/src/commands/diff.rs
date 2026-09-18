use anyhow::Result;

use aidoc::storage::crud;

use crate::session::Session;

/// v0.1 diff: list nodes that differ between two snapshots. v0.1 only
/// supports diffing against the **current** state (to=head), so we print
/// the node contents at head and at `from` (which for v0.1 also reads
/// current — full per-revision snapshot support lands with §33).
pub fn run(path: &str, from: &str, to: &str) -> Result<()> {
    let s = Session::open(path)?;
    let doc_id = s.package.as_ref().unwrap().manifest.document.id.clone();
    let head = crud::head_revision(s.store.conn(), &doc_id)?
        .ok_or_else(|| anyhow::anyhow!("no head revision"))?;

    eprintln!(
        "diff: requested {} -> {} (current head is {})",
        from, to, head
    );
    eprintln!(
        "note: v0.1 keeps only live nodes, full per-revision snapshots will land with spec §33"
    );

    let nodes = crud::list_nodes(s.store.conn(), &doc_id)?;
    println!("# Nodes at head ({}):", head);
    for n in &nodes {
        println!("  - {}: {}", n.id.as_str(), n.content);
    }
    Ok(())
}