//! Relation checks.

use std::collections::BTreeSet;

use thiserror::Error;

use aidoc_storage::{Store, crud};

use crate::ValidationReport;

#[derive(Debug, Error)]
pub enum RelationError {
    #[error(transparent)]
    Store(#[from] aidoc_storage::StoreError),
}

pub fn check(
    store: &Store,
    doc_id: &str,
    report: &mut ValidationReport,
) -> Result<(), RelationError> {
    let nodes = crud::list_nodes(store.conn(), doc_id)?;
    let ids: BTreeSet<String> = nodes.iter().map(|n| n.id.as_str().into()).collect();

    let rels = crud::list_relations(store.conn(), doc_id)?;
    for r in rels {
        if !ids.contains(r.source.as_str()) {
            report.relation_errors.push(format!(
                "relation {} has missing source {}",
                r.id,
                r.source.as_str()
            ));
        }
        if !ids.contains(r.target.as_str()) {
            report.relation_errors.push(format!(
                "relation {} has missing target {}",
                r.id,
                r.target.as_str()
            ));
        }
    }
    Ok(())
}
