//! Revision + Change (spec §23-§24).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::{NodeId, OpId, RevisionId};

/// A Change describes what actually mutated at the node level
/// (spec §24). One Operation may produce multiple Changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub id: String,
    pub revision: RevisionId,
    pub node: NodeId,
    #[serde(rename = "type")]
    pub change_type: ChangeType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<HashRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<HashRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeType {
    ContentUpdate,
    Create,
    Delete,
    Move,
    Rename,
    Split,
    Merge,
    Revert,
    RelationAdd,
    RelationRemove,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HashRef {
    pub hash: String,
}

/// A Revision represents a full document state at a logical instant.
///
/// `parents` is the **new** field for v0.1 branching (spec §34). v0.1 revisions
/// always have exactly one parent (linear history); when Branch/Merge land the
/// storage layer already accepts the multi-parent shape. Existing call sites
/// keep using `parent` for convenience; the DB column is `parents` (JSON array).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Revision {
    pub id: RevisionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<RevisionId>,
    pub operation: OpId,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Named branch this revision belongs to. None means "main".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}
