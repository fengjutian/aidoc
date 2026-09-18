//! Provenance — who created the change (spec §28-§30).

#![allow(clippy::large_enum_variant)] // Operation carries many optional AI fields.

use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ActorKind {
    Human,
    Ai,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Actor {
    #[serde(rename = "type")]
    pub kind: ActorKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// One tool/function call the AI made (name + JSON arguments + raw output).
///
/// Stored as part of AI provenance so the operation can be replayed or
/// audited. Only `name` is required; `arguments` and `output` are kept verbatim
/// from the model so downstream consumers can inspect them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolCall {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
}

/// Provenance metadata — at minimum `type`, plus typed sub-records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Provenance {
    Code {
        file: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        symbol: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        commit: Option<String>,
    },
    UserInput,
    ExternalDocument {
        uri: String,
    },
    /// Default for Human / AI / Agent edit attribution. The AI-only fields
    /// (`prompt`, `tool_calls`, `temperature`, `input_refs`, `reasoning_summary`)
    /// are spec §29 — they're optional and stay out of the HTML body.
    Operation {
        actor: Actor,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        task: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        // AI provenance — see spec §29.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        temperature: Option<f32>,
        /// IDs of nodes / revisions the AI was given as input context.
        #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
        input_refs: IndexMap<String, String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reasoning_summary: Option<String>,
    },
}

impl Provenance {
    pub fn human(id: Option<String>) -> Self {
        Self::Operation {
            actor: Actor {
                kind: ActorKind::Human,
                id,
                agent: None,
                model: None,
            },
            task: None,
            reason: None,
            prompt: None,
            tool_calls: Vec::new(),
            temperature: None,
            input_refs: IndexMap::new(),
            output: None,
            reasoning_summary: None,
        }
    }

    pub fn ai(agent: impl Into<String>, model: Option<String>) -> Self {
        Self::Operation {
            actor: Actor {
                kind: ActorKind::Ai,
                id: None,
                agent: Some(agent.into()),
                model,
            },
            task: None,
            reason: None,
            prompt: None,
            tool_calls: Vec::new(),
            temperature: None,
            input_refs: IndexMap::new(),
            output: None,
            reasoning_summary: None,
        }
    }

    /// Builder for AI provenance. Lets you attach prompt / tool calls / etc.
    /// without manually wiring each `Some(...)` site.
    pub fn ai_builder(agent: impl Into<String>, model: Option<String>) -> AiProvenanceBuilder {
        AiProvenanceBuilder {
            inner: Self::ai(agent, model),
        }
    }
}

/// Helper for constructing AI Provenance step-by-step. Each `with_*` returns
/// `self` for chaining.
pub struct AiProvenanceBuilder {
    inner: Provenance,
}

impl AiProvenanceBuilder {
    pub fn task(mut self, task: impl Into<String>) -> Self {
        if let Provenance::Operation {
            task: ref mut t, ..
        } = self.inner
        {
            *t = Some(task.into());
        }
        self
    }

    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        if let Provenance::Operation {
            reason: ref mut r, ..
        } = self.inner
        {
            *r = Some(reason.into());
        }
        self
    }

    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        if let Provenance::Operation {
            prompt: ref mut p, ..
        } = self.inner
        {
            *p = Some(prompt.into());
        }
        self
    }

    pub fn temperature(mut self, t: f32) -> Self {
        if let Provenance::Operation {
            temperature: ref mut v,
            ..
        } = self.inner
        {
            *v = Some(t);
        }
        self
    }

    pub fn reasoning_summary(mut self, summary: impl Into<String>) -> Self {
        if let Provenance::Operation {
            reasoning_summary: ref mut v,
            ..
        } = self.inner
        {
            *v = Some(summary.into());
        }
        self
    }

    pub fn tool_call(
        mut self,
        name: impl Into<String>,
        arguments: Option<serde_json::Value>,
        output: Option<serde_json::Value>,
    ) -> Self {
        if let Provenance::Operation {
            tool_calls: ref mut t,
            ..
        } = self.inner
        {
            t.push(ToolCall {
                name: name.into(),
                arguments,
                output,
            });
        }
        self
    }

    pub fn input_ref(mut self, kind: impl Into<String>, id: impl Into<String>) -> Self {
        if let Provenance::Operation {
            input_refs: ref mut m,
            ..
        } = self.inner
        {
            m.insert(kind.into(), id.into());
        }
        self
    }

    pub fn build(self) -> Provenance {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_provenance_roundtrip() {
        let p = Provenance::human(Some("alice".into()));
        let s = serde_json::to_string(&p).unwrap();
        let back: Provenance = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
        // AI-only fields must stay out of the serialized form when not set.
        assert!(!s.contains("prompt"));
        assert!(!s.contains("tool_calls"));
    }

    #[test]
    fn ai_provenance_carries_spec_29_fields() {
        let p = Provenance::ai_builder("architecture-agent", Some("MiniMax-M3".into()))
            .prompt("design order-system microservice split")
            .temperature(0.2)
            .tool_call(
                "read_node",
                Some(serde_json::json!({"id": "root"})),
                Some(serde_json::json!({"content": "..."})),
            )
            .input_ref("node", "root")
            .reasoning_summary("chose 3-service split on coupling")
            .build();
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("\"prompt\":\"design order-system"));
        assert!(s.contains("\"temperature\":0.2"));
        assert!(s.contains("\"tool_calls\""));
        assert!(s.contains("\"reasoning_summary\""));
        let back: Provenance = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }
}
