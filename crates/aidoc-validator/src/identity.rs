//! Identity checks: duplicate / invalid node IDs (spec §40).

use aidoc_storage::{Store, crud};

use crate::{Finding, ValidationCategory, ValidationError, Validator};

pub struct IdentityValidator;

impl Validator for IdentityValidator {
    fn category(&self) -> ValidationCategory {
        ValidationCategory::Identity
    }

    fn check(&self, store: &Store, doc_id: &str) -> Result<Vec<Finding>, ValidationError> {
        let nodes = crud::list_nodes(store.conn(), doc_id)?;
        let mut findings = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for n in &nodes {
            if !seen.insert(n.id.clone()) {
                findings.push(Finding::new(
                    self.category(),
                    format!("duplicate node id: {}", n.id.as_str()),
                ));
            }
            if let Err(e) = aidoc_model::id::NodeId::new(n.id.as_str()) {
                findings.push(Finding::new(
                    self.category(),
                    format!("invalid node id: {} ({})", n.id.as_str(), e),
                ));
            }
        }

        Ok(findings)
    }
}
