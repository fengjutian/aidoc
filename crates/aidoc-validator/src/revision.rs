//! Revision checks: parent exists, exactly one head.

use std::collections::BTreeSet;

use thiserror::Error;

use aidoc_storage::{Store, crud};

use crate::ValidationReport;

#[derive(Debug, Error)]
pub enum RevisionError {
    #[error(transparent)]
    Store(#[from] aidoc_storage::StoreError),
}

pub fn check(
    store: &Store,
    doc_id: &str,
    report: &mut ValidationReport,
) -> Result<(), RevisionError> {
    let revs = crud::list_revisions(store.conn(), doc_id)?;
    let ids: BTreeSet<String> = revs.iter().map(|r| r.id.as_str().into()).collect();

    for r in &revs {
        if let Some(parent) = &r.parent {
            if !ids.contains(parent.as_str()) {
                report.revision_errors.push(format!(
                    "revision {} has missing parent {}",
                    r.id.as_str(),
                    parent.as_str()
                ));
            }
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
        report.revision_errors.push(format!(
            "expected exactly 1 head revision, found {}",
            heads.len()
        ));
    }

    Ok(())
}

fn is_head(store: &Store, doc_id: &str, rev_id: &str) -> Result<bool, aidoc_storage::StoreError> {
    let head = crud::head_revision(store.conn(), doc_id)?;
    Ok(head.as_deref() == Some(rev_id))
}
