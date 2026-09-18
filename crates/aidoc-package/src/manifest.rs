//! `manifest.json` at the root of every `.aidoc` package.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
            version: "0.1".into(),
            document: ManifestDocument {
                id: doc_id.into(),
                title: title.into(),
            },
            entry: "document/document.html".into(),
            storage: ManifestStorage {
                kind: "sqlite".into(),
                path: ".internal/document.db".into(),
            },
            revision: ManifestRevision {
                current: current_rev.into(),
            },
            created_at: now,
            updated_at: now,
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