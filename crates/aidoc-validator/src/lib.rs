//! Validators (spec §40).
//!
//! Validation is pluggable: each check implements the [`Validator`] trait and
//! is registered in [`default_validators`]. Adding a new check means writing a
//! new [`Validator`] and registering it — [`validate`] itself never changes.

pub mod code_ref;
pub mod identity;
pub mod relation;
pub mod revision;
pub mod staleness;
pub mod structure;

use thiserror::Error;

use aidoc_storage::Store;

pub use staleness::{CodeState, GitCliResolver, GitResolver, Staleness, classify};

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error(transparent)]
    Store(#[from] aidoc_storage::StoreError),
}

/// The category a [`Finding`] belongs to (spec §40).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValidationCategory {
    Identity,
    Structure,
    Relation,
    Revision,
    CodeRef,
}

impl ValidationCategory {
    /// Stable label used in CLI / MCP output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Structure => "structure",
            Self::Relation => "relation",
            Self::Revision => "revision",
            Self::CodeRef => "code-ref",
        }
    }

    /// Every built-in category, in report order.
    pub fn all() -> [Self; 5] {
        [
            Self::Identity,
            Self::Structure,
            Self::Relation,
            Self::Revision,
            Self::CodeRef,
        ]
    }
}

/// A single validation issue produced by a [`Validator`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub category: ValidationCategory,
    pub message: String,
}

impl Finding {
    pub fn new(category: ValidationCategory, message: impl Into<String>) -> Self {
        Self {
            category,
            message: message.into(),
        }
    }
}

/// A pluggable document check. Implement this and register the impl in
/// [`default_validators`] to extend validation without touching [`validate`].
pub trait Validator {
    fn category(&self) -> ValidationCategory;
    fn check(&self, store: &Store, doc_id: &str) -> Result<Vec<Finding>, ValidationError>;
}

/// The built-in validator set (spec §40). Hermetic — the code-ref check runs
/// structural rules only (no git).
pub fn default_validators() -> Vec<Box<dyn Validator>> {
    validators_with(code_ref::CodeRefValidator::new())
}

/// The built-in validator set with §37 staleness enabled: the code-ref check
/// classifies each `<code-ref>` against `resolver` and reports stale/conflict
/// refs. Supply [`GitCliResolver`] to check a live checkout.
pub fn default_validators_with_code_resolver(
    resolver: Box<dyn staleness::GitResolver>,
) -> Vec<Box<dyn Validator>> {
    validators_with(code_ref::CodeRefValidator::with_resolver(resolver))
}

fn validators_with(code_ref: code_ref::CodeRefValidator) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(identity::IdentityValidator),
        Box::new(structure::StructureValidator),
        Box::new(relation::RelationValidator),
        Box::new(revision::RevisionValidator),
        Box::new(code_ref),
    ]
}

/// Run an explicit set of validators, collecting every finding.
pub fn validate_with(
    store: &Store,
    doc_id: &str,
    validators: &[Box<dyn Validator>],
) -> Result<ValidationReport, ValidationError> {
    let mut report = ValidationReport::default();
    for v in validators {
        report.findings.extend(v.check(store, doc_id)?);
    }
    Ok(report)
}

/// Run the default validator set against a document.
pub fn validate(store: &Store, doc_id: &str) -> Result<ValidationReport, ValidationError> {
    validate_with(store, doc_id, &default_validators())
}

#[derive(Debug, Default, Clone)]
pub struct ValidationReport {
    pub findings: Vec<Finding>,
}

impl ValidationReport {
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    pub fn total_errors(&self) -> usize {
        self.findings.len()
    }

    /// Findings belonging to `category`, in insertion order.
    pub fn by_category(&self, category: ValidationCategory) -> Vec<&Finding> {
        self.findings
            .iter()
            .filter(|f| f.category == category)
            .collect()
    }

    fn messages(&self, category: ValidationCategory) -> Vec<String> {
        self.by_category(category)
            .into_iter()
            .map(|f| f.message.clone())
            .collect()
    }

    // Back-compat accessors mirroring the pre-refactor report fields.
    pub fn identity_errors(&self) -> Vec<String> {
        self.messages(ValidationCategory::Identity)
    }
    pub fn structure_errors(&self) -> Vec<String> {
        self.messages(ValidationCategory::Structure)
    }
    pub fn relation_errors(&self) -> Vec<String> {
        self.messages(ValidationCategory::Relation)
    }
    pub fn revision_errors(&self) -> Vec<String> {
        self.messages(ValidationCategory::Revision)
    }
    pub fn code_ref_errors(&self) -> Vec<String> {
        self.messages(ValidationCategory::CodeRef)
    }
}
