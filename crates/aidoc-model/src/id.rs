//! Stable ID types used across the model.

use schemars::{JsonSchema, SchemaGenerator, schema::Schema};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, AIDocError>;

#[derive(Debug, Error)]
pub enum AIDocError {
    #[error("invalid node id: {0}")]
    InvalidNodeId(String),

    #[error("duplicate node id: {0}")]
    DuplicateNodeId(String),

    #[error("missing root node")]
    MissingRootNode,

    #[error("invalid parent reference: parent={parent}, child={child}")]
    InvalidParent { parent: String, child: String },

    #[error("circular hierarchy detected at node {0}")]
    CircularHierarchy(String),

    #[error("orphan node: {0}")]
    OrphanNode(String),

    #[error("missing target node: {0}")]
    MissingTarget(String),

    #[error("invalid relation: {0}")]
    InvalidRelation(String),

    #[error("revision not found: {0}")]
    RevisionNotFound(String),

    #[error("revision conflict: expected={expected}, actual={actual}")]
    RevisionConflict { expected: String, actual: String },

    #[error("invalid operation: {0}")]
    InvalidOperation(String),

    #[error("invalid target revision: {0}")]
    InvalidTargetRevision(String),

    #[error("operation failed: {0}")]
    OperationFailed(String),
}

/// A stable, human-readable node ID.
///
/// Rules from spec §8:
///   1. Unique within Document
///   2. Stable across content edits
///   3. Independent of DOM position / line numbers
///
/// We accept `[A-Za-z0-9_.-]` and require non-empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(String);

impl JsonSchema for NodeId {
    fn schema_name() -> String {
        "NodeId".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        serde_json::from_value(serde_json::json!({
            "type": "string",
            "pattern": "^[A-Za-z0-9_.-]+$",
            "minLength": 1,
            "description": "Stable node identifier (kebab-case, dot, underscore allowed)"
        }))
        .expect("NodeId schema is valid JSON")
    }
}

impl NodeId {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.is_empty() {
            return Err(AIDocError::InvalidNodeId("empty".into()));
        }
        if !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        {
            return Err(AIDocError::InvalidNodeId(value));
        }
        Ok(Self(value))
    }

    /// Internal escape hatch — caller MUST guarantee validity.
    pub fn from_validated(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// Operation ID, e.g. `OP-105`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OpId(String);

impl JsonSchema for OpId {
    fn schema_name() -> String {
        "OpId".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        serde_json::from_value(serde_json::json!({
            "type": "string",
            "description": "Operation identifier (e.g. OP-105)"
        }))
        .expect("OpId schema is valid JSON")
    }
}

impl OpId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OpId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Revision ID, e.g. `R105`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RevisionId(String);

impl JsonSchema for RevisionId {
    fn schema_name() -> String {
        "RevisionId".into()
    }
    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        serde_json::from_value(serde_json::json!({
            "type": "string",
            "pattern": "^R[0-9]+$",
            "description": "Revision identifier (e.g. R105)"
        }))
        .expect("RevisionId schema is valid JSON")
    }
}

impl RevisionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_sequence(seq: u64) -> Self {
        Self(format!("R{:03}", seq))
    }
}

impl fmt::Display for RevisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Compute a `sha256:` content hash for an arbitrary byte payload.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    format!("sha256:{}", hex::encode(digest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_accepts_valid() {
        assert!(NodeId::new("database").is_ok());
        assert!(NodeId::new("order-service").is_ok());
        assert!(NodeId::new("REQ.001").is_ok());
    }

    #[test]
    fn node_id_rejects_invalid() {
        assert!(NodeId::new("").is_err());
        assert!(NodeId::new("has space").is_err());
        assert!(NodeId::new("has/slash").is_err());
        assert!(NodeId::new("中文").is_err());
    }

    #[test]
    fn sha256_format() {
        let h = sha256_hex(b"hello");
        assert!(h.starts_with("sha256:"));
        assert_eq!(h.len(), "sha256:".len() + 64);
    }
}
