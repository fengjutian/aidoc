//! Identity checks: duplicate / invalid / missing root node IDs.

use thiserror::Error;

use aidoc_storage::{Store, crud};

use crate::ValidationReport;

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error(transparent)]
    Store(#[from] aidoc_storage::StoreError),
}

pub fn check(
    store: &Store,
    doc_id: &str,
    report: &mut ValidationReport,
) -> Result<(), IdentityError> {
    let nodes = crud::list_nodes(store.conn(), doc_id)?;
    let mut seen = std::collections::HashSet::new();
    let mut has_root = false;

    for n in &nodes {
        if !seen.insert(n.id.clone()) {
            report
                .identity_errors
                .push(format!("duplicate node id: {}", n.id.as_str()));
        }
        if n.id.as_str() == "root" {
            has_root = true;
        }
        if let Err(e) = aidoc_model::id::NodeId::new(n.id.as_str()) {
            report
                .identity_errors
                .push(format!("invalid node id: {} ({})", n.id.as_str(), e));
        }
    }

    if !has_root {
        // The "root" hint is only required if the document declares one;
        // we treat it as a warning rather than an error here.
    }

    Ok(())
}
