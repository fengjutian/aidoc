//! CodeRef checks (placeholder; we don't ship git integration in v0.1).

use thiserror::Error;

use aidoc_storage::{Store, crud};

use crate::ValidationReport;

#[derive(Debug, Error)]
pub enum CodeRefError {
    #[error(transparent)]
    Store(#[from] aidoc_storage::StoreError),
}

pub fn check(
    store: &Store,
    doc_id: &str,
    report: &mut ValidationReport,
) -> Result<(), CodeRefError> {
    let nodes = crud::list_nodes(store.conn(), doc_id)?;
    for n in nodes {
        if matches!(n.kind, aidoc_model::NodeKind::CodeRef) {
            if n.attributes
                .get("file")
                .map(|s| s.is_empty())
                .unwrap_or(true)
            {
                report
                    .code_ref_errors
                    .push(format!("code-ref {} missing file attribute", n.id.as_str()));
            }
        }
    }
    Ok(())
}
