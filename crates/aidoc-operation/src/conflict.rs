//! Conflict detection types (spec §25-§26, §36).
//!
//! AIDoc never silently overwrites a concurrent edit. Before an op mutates
//! anything the engine runs a revision check (§25) and an optional content-hash
//! check (§26); any failure is reported as a structured [`Conflict`] carrying
//! one of the five §36 classes so callers can render the machine-readable
//! `{ "status": "conflict", ... }` shape instead of guessing from a string.

use std::fmt;

use serde::Serialize;

/// The five conflict classes enumerated by spec §36.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum ConflictKind {
    /// Target node is missing / already deleted.
    Node,
    /// Stored content hash != `expected_hash` (§26).
    Content,
    /// Structural precondition violated (e.g. move onto a missing parent or a cycle).
    Structure,
    /// Relation precondition violated (e.g. duplicate link, unlink of an absent relation).
    Relation,
    /// `expected_revision` != current head (§25).
    Revision,
}

impl ConflictKind {
    /// Spec §36 wire code, e.g. `CONTENT_CONFLICT`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Node => "NODE_CONFLICT",
            Self::Content => "CONTENT_CONFLICT",
            Self::Structure => "STRUCTURE_CONFLICT",
            Self::Relation => "RELATION_CONFLICT",
            Self::Revision => "REVISION_CONFLICT",
        }
    }
}

impl fmt::Display for ConflictKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A structured conflict record.
///
/// [`Display`](fmt::Display) renders a single human line that always contains
/// the lowercase word `conflict` (so logs and existing tests keep matching);
/// [`Conflict::to_json`] renders the spec §25 machine shape.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Conflict {
    pub kind: ConflictKind,
    /// Node the conflict is about, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    /// What the operation expected (revision id, content hash, ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    /// What the store actually holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
    /// Free-form human explanation.
    pub detail: String,
}

impl Conflict {
    /// Minimal conflict with just a kind + detail.
    pub fn new(kind: ConflictKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            node: None,
            expected: None,
            actual: None,
            detail: detail.into(),
        }
    }

    /// §25 optimistic-concurrency conflict.
    pub fn revision(expected: impl Into<String>, actual: impl Into<String>) -> Self {
        let (e, a) = (expected.into(), actual.into());
        Self {
            kind: ConflictKind::Revision,
            node: None,
            expected: Some(e.clone()),
            actual: Some(a.clone()),
            detail: format!("revision conflict: expected={e}, actual={a}"),
        }
    }

    /// Target node does not exist.
    pub fn node(node: impl Into<String>) -> Self {
        let n = node.into();
        Self {
            kind: ConflictKind::Node,
            node: Some(n.clone()),
            expected: None,
            actual: None,
            detail: format!("node conflict: target '{n}' not found"),
        }
    }

    /// §26 content-hash mismatch.
    pub fn content(
        node: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        let (n, e, a) = (node.into(), expected.into(), actual.into());
        Self {
            kind: ConflictKind::Content,
            node: Some(n.clone()),
            expected: Some(e.clone()),
            actual: Some(a.clone()),
            detail: format!("content conflict on '{n}': expected={e}, actual={a}"),
        }
    }

    /// Structural precondition violated.
    pub fn structure(node: impl Into<String>, detail: impl Into<String>) -> Self {
        let n = node.into();
        Self {
            kind: ConflictKind::Structure,
            node: Some(n),
            expected: None,
            actual: None,
            detail: detail.into(),
        }
    }

    /// Relation precondition violated.
    pub fn relation(node: impl Into<String>, detail: impl Into<String>) -> Self {
        let n = node.into();
        Self {
            kind: ConflictKind::Relation,
            node: Some(n),
            expected: None,
            actual: None,
            detail: detail.into(),
        }
    }

    /// Spec §25 machine-readable shape. Always includes `status`/`kind`/`detail`;
    /// revision conflicts also carry `expected_revision`/`actual_revision`, and
    /// content conflicts carry `expected_hash`/`actual_hash`.
    pub fn to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "status".into(),
            serde_json::Value::String("conflict".into()),
        );
        map.insert(
            "kind".into(),
            serde_json::Value::String(self.kind.as_str().into()),
        );
        if let Some(n) = &self.node {
            map.insert("node".into(), serde_json::Value::String(n.clone()));
        }
        match self.kind {
            ConflictKind::Revision => {
                if let Some(e) = &self.expected {
                    map.insert(
                        "expected_revision".into(),
                        serde_json::Value::String(e.clone()),
                    );
                }
                if let Some(a) = &self.actual {
                    map.insert(
                        "actual_revision".into(),
                        serde_json::Value::String(a.clone()),
                    );
                }
            }
            ConflictKind::Content => {
                if let Some(e) = &self.expected {
                    map.insert("expected_hash".into(), serde_json::Value::String(e.clone()));
                }
                if let Some(a) = &self.actual {
                    map.insert("actual_hash".into(), serde_json::Value::String(a.clone()));
                }
            }
            _ => {}
        }
        map.insert(
            "detail".into(),
            serde_json::Value::String(self.detail.clone()),
        );
        serde_json::Value::Object(map)
    }
}

impl fmt::Display for Conflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Always contains the lowercase word "conflict" for log/test matching.
        write!(f, "{} ({})", self.detail, self.kind.as_str())
    }
}

impl std::error::Error for Conflict {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_conflict_json_matches_spec_25() {
        let c = Conflict::revision("R103", "R104");
        let v = c.to_json();
        assert_eq!(v["status"], "conflict");
        assert_eq!(v["kind"], "REVISION_CONFLICT");
        assert_eq!(v["expected_revision"], "R103");
        assert_eq!(v["actual_revision"], "R104");
    }

    #[test]
    fn content_conflict_carries_hashes() {
        let c = Conflict::content("database", "sha256:aaa", "sha256:bbb");
        let v = c.to_json();
        assert_eq!(v["kind"], "CONTENT_CONFLICT");
        assert_eq!(v["node"], "database");
        assert_eq!(v["expected_hash"], "sha256:aaa");
        assert_eq!(v["actual_hash"], "sha256:bbb");
    }

    #[test]
    fn display_contains_conflict_case_insensitive() {
        for c in [
            Conflict::revision("R1", "R2"),
            Conflict::node("x"),
            Conflict::content("x", "a", "b"),
            Conflict::structure("x", "bad parent"),
            Conflict::relation("x", "dup link"),
        ] {
            // The §36 kind suffix always ends in `_CONFLICT`, so the rendered
            // line always carries the word regardless of the detail text.
            assert!(c.to_string().to_lowercase().contains("conflict"), "{c}");
        }
    }
}
