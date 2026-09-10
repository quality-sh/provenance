use crate::{Message, Question, Requirement, Resolution, Rule, Source, StableId, Thread, Topic};
use serde::{Deserialize, Serialize};

/// Selects the Requirement viewed in a complete scope document read.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReadDocumentQuery {
    pub id: String,
}

/// Complete record collections from one projection transaction. Retired records
/// remain available for identity and reference checks. No collection is paged.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct ReadDocumentResult {
    pub root_id: StableId,
    pub requirements: Vec<Requirement>,
    pub resolutions: Vec<Resolution>,
    pub rules: Vec<Rule>,
    pub sources: Vec<Source>,
    pub topics: Vec<Topic>,
    pub questions: Vec<Question>,
    pub threads: Vec<Thread>,
    pub messages: Vec<Message>,
}
