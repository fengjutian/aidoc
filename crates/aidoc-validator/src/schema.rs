//! JSON Schema validation (spec §3.4 + §40).
//!
//! Compiles the embedded `document.schema.json` and `operation.schema.json`
//! into `jsonschema::Validator` instances at first use, then exposes
//! `validate_document_value` / `validate_operation_value` for callers.
//!
//! This complements — not replaces — the per-crate business validators
//! (structure, relation, …): schema errors are *structural* (missing required
//! fields, wrong types, additional properties not allowed), while business
//! rules catch semantic issues like duplicate node IDs or dangling relations.

use jsonschema::Validator;
use serde_json::Value;
use thiserror::Error;

const DOCUMENT_SCHEMA: &str = include_str!("../../../schemas/document.schema.json");
const OPERATION_SCHEMA: &str = include_str!("../../../schemas/operation.schema.json");

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("JSON Schema compile failure: {0}")]
    Compile(String),

    #[error("JSON Schema violation: {0}")]
    Violation(String),
}

/// Lazily-compiled validator handle. Cheap to clone.
#[derive(Debug, Clone)]
pub struct SchemaValidator {
    inner: std::sync::Arc<Validator>,
}

impl SchemaValidator {
    pub fn document() -> Self {
        static CACHE: std::sync::OnceLock<SchemaValidator> = std::sync::OnceLock::new();
        CACHE
            .get_or_init(|| {
                let schema: Value = serde_json::from_str(DOCUMENT_SCHEMA)
                    .expect("document.schema.json is compiled-in and must be valid JSON");
                let compiled = jsonschema::draft202012::new(&schema)
                    .map_err(|e| SchemaError::Compile(format!("document.schema.json: {e}")))
                    .expect("document.schema.json must compile");
                SchemaValidator {
                    inner: std::sync::Arc::new(compiled),
                }
            })
            .clone()
    }

    pub fn operation() -> Self {
        static CACHE: std::sync::OnceLock<SchemaValidator> = std::sync::OnceLock::new();
        CACHE
            .get_or_init(|| {
                let schema: Value = serde_json::from_str(OPERATION_SCHEMA)
                    .expect("operation.schema.json is compiled-in and must be valid JSON");
                let compiled = jsonschema::draft202012::new(&schema)
                    .map_err(|e| SchemaError::Compile(format!("operation.schema.json: {e}")))
                    .expect("operation.schema.json must compile");
                SchemaValidator {
                    inner: std::sync::Arc::new(compiled),
                }
            })
            .clone()
    }

    /// Validate a single JSON value against the underlying schema. On success
    /// returns `Ok(())`; on failure returns a human-readable summary of every
    /// violation (one per line) so callers can present them verbatim.
    pub fn validate(&self, value: &Value) -> Result<(), SchemaError> {
        let mut lines: Vec<String> = self
            .inner
            .iter_errors(value)
            .map(|e| format!("at {}: {}", e.instance_path(), e))
            .collect();
        if lines.is_empty() {
            // `validate` is stricter than `iter_errors` for some drafts; if
            // the iterator found nothing, double-check with `is_valid`.
            if !self.inner.is_valid(value) {
                return Err(SchemaError::Violation(
                    "value failed schema validation (no detailed errors)".into(),
                ));
            }
            return Ok(());
        }
        lines.sort();
        Err(SchemaError::Violation(lines.join("\n")))
    }
}

/// Validate a parsed canonical-document JSON value.
pub fn validate_document_value(value: &Value) -> Result<(), SchemaError> {
    SchemaValidator::document().validate(value)
}

/// Validate a parsed operation JSON value.
pub fn validate_operation_value(value: &Value) -> Result<(), SchemaError> {
    SchemaValidator::operation().validate(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn document_validator_accepts_minimal_canonical() {
        let value = json!({
            "format": "aidoc",
            "format_version": "0.2",
            "document": {
                "id": "d1",
                "title": "T",
                "root_node": "root",
            },
            "nodes": [
                {
                    "id": "root",
                    "kind": "section",
                    "position": 0,
                    "content": "Hello",
                    "attributes": {},
                }
            ],
            "relations": [],
        });
        validate_document_value(&value).expect("minimal canonical document must validate");
    }

    #[test]
    fn document_validator_rejects_missing_required() {
        let value = json!({
            "format": "aidoc",
            "format_version": "0.2",
            "document": { "id": "d1", "title": "T", "root_node": "root" },
            "nodes": [],
            // `relations` field is required and missing
        });
        let err = validate_document_value(&value).expect_err("missing relations");
        let msg = err.to_string();
        assert!(msg.contains("relations"), "got: {msg}");
    }

    #[test]
    fn document_validator_rejects_wrong_format() {
        let value = json!({
            "format": "not-aidoc",
            "format_version": "0.2",
            "document": { "id": "d1", "title": "T", "root_node": "root" },
            "nodes": [],
            "relations": [],
        });
        assert!(validate_document_value(&value).is_err());
    }

    #[test]
    fn document_validator_allows_extension_kinds() {
        // Extension types round-trip via free-form `kind` / `semantic_type`.
        // The schema only enforces that `kind` is a non-empty string.
        let value = json!({
            "format": "aidoc",
            "format_version": "0.2",
            "document": { "id": "d1", "title": "T", "root_node": "root" },
            "nodes": [
                {
                    "id": "x",
                    "kind": "com.example.chart",
                    "position": 0,
                    "content": "{}",
                    "attributes": {},
                }
            ],
            "relations": [],
        });
        validate_document_value(&value).expect("extension kinds must round-trip");
    }

    #[test]
    fn operation_validator_accepts_create() {
        let value = json!({
            "id": "op-1",
            "type": "create",
            "expected_revision": "R000",
            "actor": { "kind": "human" },
        });
        validate_operation_value(&value).expect("minimal create op must validate");
    }

    #[test]
    fn operation_validator_rejects_unknown_type() {
        let value = json!({
            "id": "op-1",
            "type": "obliterate",
            "expected_revision": "R000",
            "actor": { "kind": "human" },
        });
        assert!(validate_operation_value(&value).is_err());
    }
}