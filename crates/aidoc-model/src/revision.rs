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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Revision {
    pub id: RevisionId,
    pub parent: Option<RevisionId>,
    pub operation: OpId,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}