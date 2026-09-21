//! Semantic relation between nodes (spec §14-§15).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::id::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RelationKind {
    References,
    RelatedTo,
    DependsOn,
    Implements,
    ImplementedBy,
    DerivedFrom,
    Supersedes,
    TestedBy,
    Documents,
    DocumentsCode,
    /// Catch-all for custom relations.
    Custom,
}

impl RelationKind {
    pub fn parse(value: &str) -> Self {
        match value {
            "references" => Self::References,
            "related-to" => Self::RelatedTo,
            "depends-on" => Self::DependsOn,
            "implements" => Self::Implements,
            "implemented-by" => Self::ImplementedBy,
            "derived-from" => Self::DerivedFrom,
            "supersedes" => Self::Supersedes,
            "tested-by" => Self::TestedBy,
            "documents" => Self::Documents,
            "documents-code" => Self::DocumentsCode,
            other => {
                let _ = other;
                Self::Custom
            }
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::References => "references",
            Self::RelatedTo => "related-to",
            Self::DependsOn => "depends-on",
            Self::Implements => "implements",
            Self::ImplementedBy => "implemented-by",
            Self::DerivedFrom => "derived-from",
            Self::Supersedes => "supersedes",
            Self::TestedBy => "tested-by",
            Self::Documents => "documents",
            Self::DocumentsCode => "documents-code",
            Self::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Relation {
    pub id: String,
    pub source: NodeId,
    pub target: NodeId,
    pub kind: RelationKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_kind: Option<String>,
    /// Portable relation metadata (confidence, source location, role, etc.).
    #[serde(default, skip_serializing_if = "indexmap::IndexMap::is_empty")]
    pub attributes: indexmap::IndexMap<String, String>,
}
