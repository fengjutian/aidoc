//! Structure checks: parent exists, no cycles (spec §40).

use std::collections::{BTreeSet, HashMap};

use aidoc_storage::{Store, crud};

use crate::{Finding, ValidationCategory, ValidationError, Validator};

pub struct StructureValidator;

impl Validator for StructureValidator {
    fn category(&self) -> ValidationCategory {
        ValidationCategory::Structure
    }

    fn check(&self, store: &Store, doc_id: &str) -> Result<Vec<Finding>, ValidationError> {
        let nodes = crud::list_nodes(store.conn(), doc_id)?;
        let mut findings = Vec::new();

        let ids: BTreeSet<String> = nodes.iter().map(|n| n.id.as_str().into()).collect();
        let mut children: HashMap<String, Vec<String>> = HashMap::new();

        for n in &nodes {
            if let Some(p) = &n.parent {
                if !ids.contains(p.as_str()) {
                    findings.push(Finding::new(
                        self.category(),
                        format!("node {} has missing parent {}", n.id.as_str(), p.as_str()),
                    ));
                }
                children
                    .entry(p.as_str().into())
                    .or_default()
                    .push(n.id.as_str().into());
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
                findings.push(Finding::new(
                    self.category(),
                    format!("circular hierarchy at {}", n.id.as_str()),
                ));
                break;
            }
        }

        Ok(findings)
    }
}
