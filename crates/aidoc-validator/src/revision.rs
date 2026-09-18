//! Revision checks: parent exists, exactly one head (spec §40).

use std::collections::BTreeSet;

use aidoc_storage::{Store, crud};

use crate::{Finding, ValidationCategory, ValidationError, Validator};

pub struct RevisionValidator;

impl Validator for RevisionValidator {
    fn category(&self) -> ValidationCategory {
        ValidationCategory::Revision
    }

    fn check(&self, store: &Store, doc_id: &str) -> Result<Vec<Finding>, ValidationError> {
        let revs = crud::list_revisions(store.conn(), doc_id)?;
        let ids: BTreeSet<String> = revs.iter().map(|r| r.id.as_str().into()).collect();
        let mut findings = Vec::new();

        for r in &revs {
            if let Some(parent) = &r.parent
                && !ids.contains(parent.as_str())
            {
                findings.push(Finding::new(
                    self.category(),
                    format!(
                        "revision {} has missing parent {}",
                        r.id.as_str(),
                        parent.as_str()
                    ),
                ));
            }
        }

        let heads: Vec<&str> = revs
            .iter()
            .filter_map(|r| {
                let s: &str = r.id.as_str();
                if is_head(store, doc_id, s).unwrap_or(false) {
                    Some(s)
                } else {
                    None
                }
            })
            .collect();

        if heads.len() != 1 {
            findings.push(Finding::new(
                self.category(),
                format!("expected exactly 1 head revision, found {}", heads.len()),
            ));
        }

        Ok(findings)
    }
}

fn is_head(store: &Store, doc_id: &str, rev_id: &str) -> Result<bool, aidoc_storage::StoreError> {
    let head = crud::head_revision(store.conn(), doc_id)?;
    Ok(head.as_deref() == Some(rev_id))
}
