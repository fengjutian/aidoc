//! CodeRef staleness (spec §16-§17, §37).
//!
//! A `<code-ref>` records the git `commit` it was last synced at (§16). When
//! the referenced file moves on to a different commit the reference is *stale*
//! — the prose may no longer describe the code (§17). Spec §37 defines four
//! states: `synced / stale / conflict / unknown`.
//!
//! The computation lives behind an injectable [`GitResolver`] seam so the
//! classification logic is testable without a real repository, and the default
//! validator set stays hermetic (no resolver ⇒ no `git` invocation). Wire in
//! [`GitCliResolver`] (or your own impl) to check against a live checkout.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The four §37 staleness states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Staleness {
    /// Recorded commit matches the file's current commit.
    Synced,
    /// The file moved to a different commit than the one recorded.
    Stale,
    /// The resolver reports a divergent / conflicting state.
    Conflict,
    /// Not enough information: no commit recorded, no resolver, or git could not
    /// resolve the file.
    Unknown,
}

impl Staleness {
    /// Stable label used in CLI / findings output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Synced => "synced",
            Self::Stale => "stale",
            Self::Conflict => "conflict",
            Self::Unknown => "unknown",
        }
    }

    /// True when the state warrants a validation finding (i.e. the doc may be
    /// out of date). `synced` and `unknown` are not actionable.
    pub fn is_actionable(self) -> bool {
        matches!(self, Self::Stale | Self::Conflict)
    }
}

impl std::fmt::Display for Staleness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a [`GitResolver`] knows about a referenced file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeState {
    /// The file's latest commit hash.
    AtCommit(String),
    /// The file is in a divergent / conflicting state (e.g. unmerged).
    Conflicted,
    /// Cannot be determined (no git, untracked file, resolver absent).
    Unknown,
}

/// Seam for resolving a file's current git state. Implement this to plug in a
/// different VCS — or a fake for tests. [`GitCliResolver`] shells out to `git`.
pub trait GitResolver {
    fn state(&self, file: &str) -> CodeState;
}

/// Classify a CodeRef per §37.
///
/// `recorded_commit` is the `commit` attribute stored on the node; `file` is
/// its `file` attribute. Returns [`Staleness::Unknown`] whenever there is not
/// enough information to decide, so callers never mistake "can't tell" for
/// "in sync".
pub fn classify(
    recorded_commit: Option<&str>,
    file: &str,
    resolver: Option<&dyn GitResolver>,
) -> Staleness {
    let (Some(resolver), Some(recorded)) = (resolver, recorded_commit) else {
        return Staleness::Unknown;
    };
    if recorded.trim().is_empty() {
        return Staleness::Unknown;
    }
    match resolver.state(file) {
        CodeState::AtCommit(cur) => {
            if commit_eq(&cur, recorded) {
                Staleness::Synced
            } else {
                Staleness::Stale
            }
        }
        CodeState::Conflicted => Staleness::Conflict,
        CodeState::Unknown => Staleness::Unknown,
    }
}

/// Commits compare equal when identical or when one is a prefix of the other,
/// so a short SHA (`9f910ed`) matches the full hash git reports. Prefix
/// matching only kicks in at ≥7 chars to avoid spurious short-prefix hits.
fn commit_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim(), b.trim());
    if a == b {
        return true;
    }
    a.len() >= 7 && b.len() >= 7 && (a.starts_with(b) || b.starts_with(a))
}

/// [`GitResolver`] backed by the `git` CLI, rooted at a repository path.
///
/// Resolves a file's state with `git -C <repo> log -1 --format=%H -- <file>`.
/// Any failure (no git binary, not a repo, untracked file, empty output) reads
/// as [`CodeState::Unknown`] so validation degrades gracefully instead of
/// erroring.
pub struct GitCliResolver {
    repo: PathBuf,
}

impl GitCliResolver {
    pub fn new(repo: impl Into<PathBuf>) -> Self {
        Self { repo: repo.into() }
    }

    pub fn repo(&self) -> &Path {
        &self.repo
    }
}

impl GitResolver for GitCliResolver {
    fn state(&self, file: &str) -> CodeState {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.repo)
            .args(["log", "-1", "--format=%H", "--"])
            .arg(file)
            .output();
        match out {
            Ok(o) if o.status.success() => {
                let hash = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if hash.is_empty() {
                    CodeState::Unknown
                } else {
                    CodeState::AtCommit(hash)
                }
            }
            _ => CodeState::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct FakeResolver(HashMap<String, CodeState>);

    impl GitResolver for FakeResolver {
        fn state(&self, file: &str) -> CodeState {
            self.0.get(file).cloned().unwrap_or(CodeState::Unknown)
        }
    }

    fn fake(pairs: &[(&str, CodeState)]) -> FakeResolver {
        FakeResolver(
            pairs
                .iter()
                .map(|(f, s)| (f.to_string(), s.clone()))
                .collect(),
        )
    }

    const FULL: &str = "9f910ed1234567890abcdef1234567890abcdef1";

    #[test]
    fn synced_when_commits_match() {
        let r = fake(&[("src/a.ts", CodeState::AtCommit(FULL.into()))]);
        assert_eq!(classify(Some(FULL), "src/a.ts", Some(&r)), Staleness::Synced);
    }

    #[test]
    fn short_sha_prefix_counts_as_synced() {
        let r = fake(&[("src/a.ts", CodeState::AtCommit(FULL.into()))]);
        assert_eq!(
            classify(Some("9f910ed"), "src/a.ts", Some(&r)),
            Staleness::Synced
        );
    }

    #[test]
    fn stale_when_commit_differs() {
        let r = fake(&[("src/a.ts", CodeState::AtCommit("ffffffffffffffff".into()))]);
        assert_eq!(classify(Some(FULL), "src/a.ts", Some(&r)), Staleness::Stale);
    }

    #[test]
    fn conflict_state_maps_through() {
        let r = fake(&[("src/a.ts", CodeState::Conflicted)]);
        assert_eq!(
            classify(Some(FULL), "src/a.ts", Some(&r)),
            Staleness::Conflict
        );
    }

    #[test]
    fn unknown_without_resolver() {
        assert_eq!(classify(Some(FULL), "src/a.ts", None), Staleness::Unknown);
    }

    #[test]
    fn unknown_without_recorded_commit() {
        let r = fake(&[("src/a.ts", CodeState::AtCommit(FULL.into()))]);
        assert_eq!(classify(None, "src/a.ts", Some(&r)), Staleness::Unknown);
        assert_eq!(
            classify(Some("  "), "src/a.ts", Some(&r)),
            Staleness::Unknown
        );
    }

    #[test]
    fn unknown_when_file_unresolved() {
        let r = fake(&[]);
        assert_eq!(
            classify(Some(FULL), "missing.ts", Some(&r)),
            Staleness::Unknown
        );
    }

    #[test]
    fn actionable_only_for_stale_and_conflict() {
        assert!(Staleness::Stale.is_actionable());
        assert!(Staleness::Conflict.is_actionable());
        assert!(!Staleness::Synced.is_actionable());
        assert!(!Staleness::Unknown.is_actionable());
    }
}
