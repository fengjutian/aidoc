use anyhow::Result;

use aidoc::validator::{self, ValidationCategory};

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

    for cat in ValidationCategory::all() {
        let findings = report.by_category(cat);
        if findings.is_empty() {
            continue;
        }
        println!("[{}] {}", cat.as_str(), findings.len());
        for f in &findings {
            println!("  - {}", f.message);
        }
    }
    let total = report.total_errors();
    println!("total: {total} error(s)");
    Err(anyhow::Error::new(ValidateFailure { count: total }))
}
