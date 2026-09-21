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

    /// Upgrade package metadata without discarding v0.1 content/history.
    pub fn upgrade_to_v02(&mut self) {
        self.version = "0.2".into();
        self.entry = "document/document.json".into();
        self.schemas
            .insert("document".into(), "schemas/document.schema.json".into());
        self.schemas
            .insert("operation".into(), "schemas/operation.schema.json".into());
        self.representations
            .insert("html".into(), "document/document.html".into());
        for capability in [
            "nodes",
            "relations",
            "operations",
            "revisions",
            "extensions",
        ] {
            if !self.capabilities.iter().any(|item| item == capability) {
                self.capabilities.push(capability.into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v01_manifest_deserializes_and_upgrades_without_losing_identity() {
        let raw = r#"{
          "format":"aidoc","version":"0.1",
          "document":{"id":"demo","title":"Demo"},
          "entry":"document/document.html",
          "storage":{"type":"sqlite","path":".internal/document.db"},
          "revision":{"current":"R009"},
          "created_at":"2026-01-01T00:00:00Z",
          "updated_at":"2026-01-01T00:00:00Z"
        }"#;
        let mut manifest: Manifest = serde_json::from_str(raw).unwrap();
        manifest.upgrade_to_v02();
        assert_eq!(manifest.document.id, "demo");
        assert_eq!(manifest.revision.current, "R009");
        assert_eq!(manifest.version, "0.2");
        assert_eq!(manifest.entry, "document/document.json");
        assert!(manifest.schemas.contains_key("operation"));
    }
}
