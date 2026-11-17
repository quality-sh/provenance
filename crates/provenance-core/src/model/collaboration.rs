use serde::{Deserialize, Serialize};

use super::graph::NodeType;
use super::ids::{ScopeId, StableId};
use super::parsing::parse_enum_word;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ThreadStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "resolved")]
    Resolved,
    #[serde(rename = "archived")]
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum MessageRole {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "system")]
    System,
}

impl MessageRole {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        parse_enum_word(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ThreadParent {
    pub node_type: NodeType,
    pub node_id: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Thread {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub parent: ThreadParent,
    pub status: ThreadStatus,
    #[serde(deserialize_with = "super::exact_integer::time")]
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Message {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub thread_id: StableId,
    pub role: MessageRole,
    /// The actor of the write that added the Message.
    #[serde(deserialize_with = "non_blank_actor")]
    pub actor: String,
    #[serde(alias = "content")]
    pub body: String,
    #[serde(deserialize_with = "super::exact_integer::time")]
    pub created_at: i64,
}

/// Reads the actor of a Message and refuses a blank actor, as serde refuses a missing one.
fn non_blank_actor<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let actor = String::deserialize(deserializer)?;
    if actor.trim().is_empty() {
        return Err(serde::de::Error::custom("the field `actor` is blank"));
    }
    Ok(actor)
}
