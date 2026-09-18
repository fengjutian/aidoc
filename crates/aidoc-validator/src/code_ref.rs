//! CodeRef checks (spec §16-§17, §37).
//!
//! Two levels:
//!   * **Structural** (always on): a `<code-ref>` must carry a `file`.
//!   * **Staleness** (opt-in): when constructed with a [`GitResolver`], each
//!     ref's recorded `commit` is compared against the file's current commit
//!     and classified per §37. Actionable states (`stale` / `conflict`) become
//!     findings. Without a resolver the check stays hermetic — no git, no I/O.

use aidoc_storage::{Store, crud};

use crate::staleness::{GitResolver, classify};
use crate::{Finding, ValidationCategory, ValidationError, Validator};

pub struct CodeRefValidator {
    resolver: Option<Box<dyn GitResolver>>,
}

impl CodeRefValidator {
    /// Hermetic default — structural checks only.
    pub fn new() -> Self {
        Self { resolver: None }
    }

    /// Enable §37 staleness classification using `resolver`.
    pub fn with_resolver(resolver: Box<dyn GitResolver>) -> Self {
        Self {
            resolver: Some(resolver),
        }
    }
}

impl Default for CodeRefValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator for CodeRefValidator {
    fn category(&self) -> ValidationCategory {
        ValidationCategory::CodeRef
    }

    fn check(&self, store: &Store, doc_id: &str) -> Result<Vec<Finding>, ValidationError> {
        let nodes = crud::list_nodes(store.conn(), doc_id)?;
        let mut findings = Vec::new();
        for n in nodes {
            if !matches!(n.kind, aidoc_model::NodeKind::CodeRef) {
                continue;
            }
            let file = n.attributes.get("file").cloned().unwrap_or_default();
            if file.is_empty() {
                findings.push(Finding::new(
                    self.category(),
                    format!("code-ref {} missing file attribute", n.id.as_str()),
                ));
                continue;
            }
            // Staleness (§37) only when a resolver was supplied.
            if let Some(resolver) = &self.resolver {
                let recorded = n.attributes.get("commit").map(|s| s.as_str());
                let state = classify(recorded, &file, Some(resolver.as_ref()));
                if state.is_actionable() {
                    findings.push(Finding::new(
                        self.category(),
                        format!(
                            "code-ref {} ({file}) is {state}: recorded commit {}, code has moved on",
                            n.id.as_str(),
                            recorded.unwrap_or("<none>")
                        ),
                    ));
                }
            }
        }
        Ok(findings)
    }
}
