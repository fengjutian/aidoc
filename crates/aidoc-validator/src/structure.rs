//! Structure checks: parent exists, no cycles, no orphans.

use std::collections::{BTreeSet, HashMap};

use thiserror::Error;

use aidoc_storage::{crud, Store};

use crate::ValidationReport;

#[derive(Debug, Error)]
pub enum StructureError {
    #[error(transparent)]
    Store(#[from] aidoc_storage::StoreError),
}

pub fn check(store: &Store, doc_id: &str, report: &mut ValidationReport) -> Result<(), StructureError> {
    let nodes = crud::list_nodes(store.conn(), doc_id)?;

    let ids: BTreeSet<String> = nodes.iter().map(|n| n.id.as_str().into()).collect();
    let mut children: HashMap<String, Vec<String>> = HashMap::new();

    for n in &nodes {
        if let Some(p) = &n.parent {
            if !ids.contains(p.as_str()) {
                report.structure_errors.push(format!(
                    "node {} has missing parent {}",
                    n.id.as_str(),
                    p.as_str()
                ));
            }
            children.entry(p.as_str().into()).or_default().push(n.id.as_str().into());
        }
    }

    // Cycle detection — DFS.
    fn dfs(
        node: &str,
        children: &HashMap<String, Vec<String>>,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        if visiting.contains(node) {
            return true; // cycle
        }
        if visited.contains(node) {
            return false;
        }
        visiting.insert(node.into());
        if let Some(kids) = children.get(node) {
            for k in kids {
                if dfs(k, children, visiting, visited) {
                    return true;
                }
            }
        }
        visiting.remove(node);
        visited.insert(node.into());
        false
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for n in &nodes {
        if dfs(n.id.as_str(), &children, &mut visiting, &mut visited) {
            report.structure_errors.push(format!("circular hierarchy at {}", n.id.as_str()));
            break;
        }
    }

    Ok(())
}