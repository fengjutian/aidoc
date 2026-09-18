use anyhow::Result;

use aidoc::validator;

use crate::session::Session;

/// Errors produced by `aidoc validate`. Carries the total number of issues
/// found across the 5 validator categories so `main.rs` can translate it
/// into a process exit code.
#[derive(Debug)]
pub struct ValidateFailure {
    pub count: usize,
}

impl std::fmt::Display for ValidateFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} validation error(s)", self.count)
    }
}

impl std::error::Error for ValidateFailure {}

pub fn run(path: &str) -> Result<()> {
    let s = Session::open(path)?;
    let doc_id = s.package.as_ref().unwrap().manifest.document.id.clone();
    let report = validator::validate(&s.store, &doc_id)?;

    if report.is_clean() {
        println!("OK: 0 errors across identity/structure/relation/revision/code-ref");
        return Ok(());
    }

    if !report.identity_errors.is_empty() {
        println!("[identity] {}", report.identity_errors.len());
        for e in &report.identity_errors {
            println!("  - {e}");
        }
    }
    if !report.structure_errors.is_empty() {
        println!("[structure] {}", report.structure_errors.len());
        for e in &report.structure_errors {
            println!("  - {e}");
        }
    }
    if !report.relation_errors.is_empty() {
        println!("[relation] {}", report.relation_errors.len());
        for e in &report.relation_errors {
            println!("  - {e}");
        }
    }
    if !report.revision_errors.is_empty() {
        println!("[revision] {}", report.revision_errors.len());
        for e in &report.revision_errors {
            println!("  - {e}");
        }
    }
    if !report.code_ref_errors.is_empty() {
        println!("[code-ref] {}", report.code_ref_errors.len());
        for e in &report.code_ref_errors {
            println!("  - {e}");
        }
    }
    let total = report.total_errors();
    println!("total: {total} error(s)");
    Err(anyhow::Error::new(ValidateFailure { count: total }))
}