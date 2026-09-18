//! Relation checks: source / target must exist (spec §40).

use std::collections::BTreeSet;

use aidoc_storage::{Store, crud};

use crate::{Finding, ValidationCategory, ValidationError, Validator};

pub struct RelationValidator;

impl Validator for RelationValidator {
    fn category(&self) -> ValidationCategory {
        ValidationCategory::Relation
    }

    fn check(&self, store: &Store, doc_id: &str) -> Result<Vec<Finding>, ValidationError> {
        let nodes = crud::list_nodes(store.conn(), doc_id)?;
        let ids: BTreeSet<String> = nodes.iter().map(|n| n.id.as_str().into()).collect();
        let mut findings = Vec::new();

        let rels = crud::list_relations(store.conn(), doc_id)?;
        for r in rels {
            if !ids.contains(r.source.as_str()) {
                findings.push(Finding::new(
                    self.category(),
                    format!("relation {} has missing source {}", r.id, r.source.as_str()),
                ));
            }
            if !ids.contains(r.target.as_str()) {
                findings.push(Finding::new(
                    self.category(),
                    format!("relation {} has missing target {}", r.id, r.target.as_str()),
                ));
            }
        }

        Ok(findings)
    }
}
