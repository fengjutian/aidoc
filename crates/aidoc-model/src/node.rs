//! Node model (spec §7, §8, §10-§12).

use serde::{Deserialize, Serialize};

use crate::id::NodeId;

/// All node kinds supported in v0.1.
///
/// Mapping to HTML:
///   - structural (section/article/h1..h6/...) → `Section` kind
///   - p → `Paragraph`
///   - code/pre → `Code`
///   - ul/ol/li → `List` / `ListItem`
///   - table/tr/td → `Table`/`TableCell`
///   - blockquote → `Blockquote`
///   - <diagram> → `Diagram`
///   - <code-ref> → `CodeRef`
///   - <requirement> / <decision> / <problem> / <solution> → respective
///   - <ref> → `Reference`
///   - a → `Link`
///   - img → `Image`
///   - details/summary → `Details` / `Summary`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NodeKind {
    Section,
    Paragraph,
    Heading,
    List,
    ListItem,
    Table,
    TableRow,
    TableCell,
    Code,
    Blockquote,
    Link,
    Image,
    Diagram,
    CodeRef,
    Requirement,
    Decision,
    Problem,
    Solution,
    Reference,
    Details,
    Summary,
    Generic,
}

impl NodeKind {
    pub fn is_block(&self) -> bool {
        matches!(
            self,
            Self::Section
                | Self::Paragraph
                | Self::Heading
                | Self::List
                | Self::Table
                | Self::Code
                | Self::Blockquote
                | Self::Diagram
                | Self::CodeRef
                | Self::Requirement
                | Self::Decision
                | Self::Problem
                | Self::Solution
                | Self::Details
                | Self::Generic
        )
    }
}

/// A Node is the atomic unit of the document.
///
/// `content` holds the textual / inline HTML payload for leaf nodes.
/// `position` is the sibling order under `parent` (0-indexed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub parent: Option<NodeId>,
    pub position: u32,
    /// Optional semantic tag from `data-aidoc-type="..."`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_type: Option<String>,
    /// Plain or inline-HTML payload. For block nodes this is the body.
    #[serde(default)]
    pub content: String,
    /// Free-form attributes preserved on the node (e.g. `<code-ref file=...>`).
    #[serde(default)]
    pub attributes: indexmap::IndexMap<String, String>,
}

impl Node {
    pub fn new(id: impl Into<NodeId>, kind: NodeKind) -> Self {
        Self {
            id: id.into(),
            kind,
            parent: None,
            position: 0,
            semantic_type: None,
            content: String::new(),
            attributes: indexmap::IndexMap::new(),
        }
    }
}
