//! `manifest.json` at the root of every `.aidoc` package.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub version: String,
    pub document: ManifestDocument,
    pub entry: String,
    pub storage: ManifestStorage,
    pub revision: ManifestRevision,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Machine-readable schemas shipped inside the package. Added in v0.2;
    /// defaults keep v0.1 packages readable.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub schemas: BTreeMap<String, String>,
    /// Alternative, derived representations. The canonical entry remains JSON.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub representations: BTreeMap<String, String>,
    /// Feature negotiation for agents and generic consumers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestDocument {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestStorage {
    #[serde(rename = "type")]
    pub kind: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestRevision {
    pub current: String,
}

impl Manifest {
    pub fn new(
        doc_id: impl Into<String>,
        title: impl Into<String>,
        current_rev: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            format: "aidoc".into(),
            version: "0.2".into(),
            document: ManifestDocument {
                id: doc_id.into(),
                title: title.into(),
            },
            entry: "document/document.json".into(),
            storage: ManifestStorage {
                kind: "sqlite".into(),
                path: ".internal/document.db".into(),
            },
            revision: ManifestRevision {
                current: current_rev.into(),
            },
            created_at: now,
            updated_at: now,
            schemas: BTreeMap::from([
                ("document".into(), "schemas/document.schema.json".into()),
                ("operation".into(), "schemas/operation.schema.json".into()),
            ]),
            representations: BTreeMap::from([("html".into(), "document/document.html".into())]),
            capabilities: vec![
                "nodes".into(),
                "relations".into(),
                "operations".into(),
                "revisions".into(),
                "extensions".into(),
            ],
        }
    }

    pub fn touch_updated(&mut self) {
        self.updated_at = Utc::now();
    }

    pub fn set_revision(&mut self, rev: impl Into<String>) {
        self.revision.current = rev.into();
        self.touch_updated();
    }
}
