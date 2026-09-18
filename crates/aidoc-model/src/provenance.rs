//! Provenance — who created the change (spec §28-§30).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActorKind {
    Human,
    Ai,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Provenance metadata — at minimum `type`, plus typed sub-records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Default for Human / AI / Agent edit attribution.
    Operation {
        actor: Actor,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        task: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
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
        }
    }
}