//! CodeRef checks (placeholder; git integration is not shipped in v0.1).

use aidoc_storage::{Store, crud};

use crate::{Finding, ValidationCategory, ValidationError, Validator};

pub struct CodeRefValidator;

impl Validator for CodeRefValidator {
    fn category(&self) -> ValidationCategory {
        ValidationCategory::CodeRef
    }

    fn check(&self, store: &Store, doc_id: &str) -> Result<Vec<Finding>, ValidationError> {
        let nodes = crud::list_nodes(store.conn(), doc_id)?;
        let mut findings = Vec::new();
        for n in nodes {
            if matches!(n.kind, aidoc_model::NodeKind::CodeRef)
                && n.attributes
                    .get("file")
                    .map(|s| s.is_empty())
                    .unwrap_or(true)
            {
                findings.push(Finding::new(
                    self.category(),
                    format!("code-ref {} missing file attribute", n.id.as_str()),
                ));
            }
        }
        Ok(findings)
    }
}
