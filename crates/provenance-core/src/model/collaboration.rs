use serde::{Deserialize, Serialize};

use super::graph::NodeType;
use super::ids::{SchemaVersion, ScopeId, StableId};

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
        match value {
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            "system" => Ok(Self::System),
            _ => anyhow::bail!("role must be user, assistant, or system"),
        }
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
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    pub parent: ThreadParent,
    pub status: ThreadStatus,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Message {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    pub thread_id: StableId,
    pub role: MessageRole,
    #[serde(alias = "content")]
    pub body: String,
    pub created_at: i64,
    #[serde(default, alias = "aiMetadata", skip_serializing_if = "Option::is_none")]
    pub ai_metadata: Option<serde_json::Value>,
}
