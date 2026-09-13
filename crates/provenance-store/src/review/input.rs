use crate::state_store::UpdateRequirementInput;
use provenance_core::{SourceReference, StableId};
use serde::{Deserialize, Serialize};

/// Relationships are replaced and validated as one final set.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementRelations {
    pub refines: Option<StableId>,
    pub depends_on: Vec<StableId>,
    pub supersedes: Vec<StableId>,
    pub spawned_by: Option<StableId>,
    pub source_refs: Vec<SourceReference>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveRequirement {
    pub request_id: StableId,
    pub actor: String,
    pub expected_etag: String,
    pub update: UpdateRequirementInput,
    pub relationships: Option<RequirementRelations>,
}
