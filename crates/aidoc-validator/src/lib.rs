//! Validators (spec §40).

pub mod code_ref;
pub mod identity;
pub mod relation;
pub mod revision;
pub mod structure;

use thiserror::Error;

use aidoc_storage::Store;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error(transparent)]
    Identity(#[from] identity::IdentityError),

    #[error(transparent)]
    Structure(#[from] structure::StructureError),

    #[error(transparent)]
    Relation(#[from] relation::RelationError),

    #[error(transparent)]
    Revision(#[from] revision::RevisionError),

    #[error(transparent)]
    CodeRef(#[from] code_ref::CodeRefError),

    #[error(transparent)]
    Store(#[from] aidoc_storage::StoreError),
}

pub fn validate(store: &Store, doc_id: &str) -> Result<ValidationReport, ValidationError> {
    let mut report = ValidationReport::default();

    identity::check(store, doc_id, &mut report)?;
    structure::check(store, doc_id, &mut report)?;
    relation::check(store, doc_id, &mut report)?;
    revision::check(store, doc_id, &mut report)?;
    code_ref::check(store, doc_id, &mut report)?;

    Ok(report)
}

#[derive(Debug, Default, Clone)]
pub struct ValidationReport {
    pub identity_errors: Vec<String>,
    pub structure_errors: Vec<String>,
    pub relation_errors: Vec<String>,
    pub revision_errors: Vec<String>,
    pub code_ref_errors: Vec<String>,
}

impl ValidationReport {
    pub fn is_clean(&self) -> bool {
        self.identity_errors.is_empty()
            && self.structure_errors.is_empty()
            && self.relation_errors.is_empty()
            && self.revision_errors.is_empty()
            && self.code_ref_errors.is_empty()
    }

    pub fn total_errors(&self) -> usize {
        self.identity_errors.len()
            + self.structure_errors.len()
            + self.relation_errors.len()
            + self.revision_errors.len()
            + self.code_ref_errors.len()
    }
}