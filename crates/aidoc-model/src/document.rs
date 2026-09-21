//! Document root (spec §6).

use serde::{Deserialize, Serialize};

use crate::id::NodeId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub root_node: NodeId,
    #[serde(default = "default_version")]
    pub version: String,
}

fn default_version() -> String {
    "0.2".into()
}

impl Document {
    pub fn new(id: impl Into<String>, title: impl Into<String>, root: impl Into<NodeId>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            root_node: root.into(),
            version: default_version(),
        }
    }
}
