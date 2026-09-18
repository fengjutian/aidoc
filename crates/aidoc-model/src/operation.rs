//! Operation + OperationType (spec §20-§22).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::id::{NodeId, OpId, RevisionId};
use crate::provenance::Provenance;

/// Spec §21 — supported v0.1 operation types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationType {
    Create,
    Update,
    Delete,
    Move,
    Rename,
    Replace,
    Link,
    Unlink,
    Split,
    Merge,
    Revert,
}

impl OperationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Move => "move",
            Self::Rename => "rename",
            Self::Replace => "replace",
            Self::Link => "link",
            Self::Unlink => "unlink",
            Self::Split => "split",
            Self::Merge => "merge",
            Self::Revert => "revert",
        }
    }
}

/// Patch body — a partial update to a node.
///
/// We model patches as a small typed struct rather than free-form JSON to
/// keep the operation surface tight. Extend by adding fields, never by
/// replacing the whole document.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_type: Option<String>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub attributes: indexmap::IndexMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    pub id: OpId,
    #[serde(rename = "type")]
    pub op_type: OperationType,
    /// Primary target node (or for split: source).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<NodeId>,
    /// Optimistic-concurrency check.
    pub expected_revision: RevisionId,
    /// For revert only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_revision: Option<RevisionId>,
    /// For split: list of resulting targets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<NodeId>,
    pub actor: Provenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<Patch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
