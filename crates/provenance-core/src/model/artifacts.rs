use provenance_macros::Relations;
use serde::{Deserialize, Serialize};

use super::ids::{SchemaVersion, ScopeId, StableId};
use super::validation::{
    deserialize_optional_commit_pin, deserialize_optional_confidence,
    validate_resolution_input_content,
};

mod kinds;

pub use kinds::{
    RequirementStatus, ResolutionInputType, ResolutionStatus, RuleSeverity, RuleStatus, SourceType,
};

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Relations)]
pub struct Source {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration_address: Option<super::DeclarationAddress>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub retired: bool,
    pub name: String,
    #[serde(alias = "sourceType")]
    pub source_type: SourceType,
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(
        default,
        alias = "commitPin",
        deserialize_with = "deserialize_optional_commit_pin",
        skip_serializing_if = "Option::is_none"
    )]
    pub commit_pin: Option<String>,
    #[serde(
        default,
        alias = "effectiveDate",
        skip_serializing_if = "Option::is_none"
    )]
    pub effective_date: Option<i64>,
    #[serde(default, alias = "reviewDate", skip_serializing_if = "Option::is_none")]
    pub review_date: Option<i64>,
    #[relation(target = Source, flow = target_downstream)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supersedes: Vec<StableId>,
    #[relation(none)]
    #[serde(
        default,
        alias = "supersededBy",
        skip_serializing_if = "Option::is_none"
    )]
    pub superseded_by: Option<StableId>,
    #[relation(none)]
    #[serde(
        default,
        alias = "originThread",
        skip_serializing_if = "Option::is_none"
    )]
    pub origin_thread: Option<StableId>,
    #[relation(none)]
    #[serde(
        default,
        alias = "originMessage",
        skip_serializing_if = "Option::is_none"
    )]
    pub origin_message: Option<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceReference {
    #[serde(alias = "sourceId")]
    pub source_id: StableId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clause: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Relations)]
pub struct Requirement {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration_address: Option<super::DeclarationAddress>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub retired: bool,
    pub statement: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Deliberately unstructured free text: the dim view of decisions and
    /// investigations that are coming but cannot yet be phrased sharply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fog: Option<String>,
    pub status: RequirementStatus,
    #[relation(target = Domain, flow = none)]
    #[serde(default, alias = "domainId", skip_serializing_if = "Option::is_none")]
    pub domain_id: Option<StableId>,
    #[relation(target = Source, flow = target_upstream, name = "cites", via = source_id)]
    #[serde(default, alias = "sourceRefs", skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<SourceReference>,
    #[relation(target = Requirement, flow = target_upstream)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refines: Option<StableId>,
    #[relation(target = Requirement, flow = target_downstream)]
    #[serde(default, alias = "dependsOn", skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<StableId>,
    #[relation(target = Requirement, flow = target_downstream)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supersedes: Vec<StableId>,
    #[relation(target = Resolution, flow = target_upstream)]
    #[serde(default, alias = "spawnedBy", skip_serializing_if = "Option::is_none")]
    pub spawned_by: Option<StableId>,
    #[relation(none)]
    #[serde(
        default,
        alias = "originThread",
        skip_serializing_if = "Option::is_none"
    )]
    pub origin_thread: Option<StableId>,
    #[relation(none)]
    #[serde(
        default,
        alias = "originMessage",
        skip_serializing_if = "Option::is_none"
    )]
    pub origin_message: Option<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ResolutionInputFields")]
pub struct ResolutionInput {
    pub input_type: ResolutionInputType,
    pub reference: String,
    pub summary: String,
}

/// The fields of a [`ResolutionInput`] as they arrive on the wire, before
/// `validate_resolution_input_content` has passed judgement on them. Serde
/// reads this, the conversion below either builds the record or refuses it, so
/// a blank input cannot enter the graph through a file the way it can through
/// a struct literal.
#[derive(Deserialize)]
struct ResolutionInputFields {
    #[serde(alias = "inputType")]
    input_type: ResolutionInputType,
    reference: String,
    summary: String,
}

impl TryFrom<ResolutionInputFields> for ResolutionInput {
    type Error = anyhow::Error;

    fn try_from(fields: ResolutionInputFields) -> anyhow::Result<Self> {
        validate_resolution_input_content(&fields.reference, &fields.summary)?;
        Ok(Self {
            input_type: fields.input_type,
            reference: fields.reference,
            summary: fields.summary,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Relations)]
pub struct Resolution {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    pub title: String,
    pub position: String,
    pub rationale: String,
    pub status: ResolutionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforcement: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_confidence",
        skip_serializing_if = "Option::is_none"
    )]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub inputs: Vec<ResolutionInput>,
    #[serde(default, alias = "madeBy", skip_serializing_if = "Option::is_none")]
    pub made_by: Option<String>,
    #[serde(default, alias = "approvedBy", skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<String>,
    #[serde(default, alias = "approvedAt", skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<i64>,
    #[relation(target = Requirement, flow = target_upstream, required)]
    #[serde(
        default,
        alias = "requirementIds",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub requirement_ids: Vec<StableId>,
    #[relation(target = Resolution, flow = target_downstream)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supersedes: Vec<StableId>,
    #[relation(none)]
    #[serde(
        default,
        alias = "supersededBy",
        skip_serializing_if = "Option::is_none"
    )]
    pub superseded_by: Option<StableId>,
    #[serde(alias = "reviewOn")]
    pub review_on: Option<String>,
    #[relation(none)]
    #[serde(
        default,
        alias = "originThread",
        skip_serializing_if = "Option::is_none"
    )]
    pub origin_thread: Option<StableId>,
    #[relation(none)]
    #[serde(
        default,
        alias = "originMessage",
        skip_serializing_if = "Option::is_none"
    )]
    pub origin_message: Option<StableId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Relations)]
pub struct Rule {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration_address: Option<super::DeclarationAddress>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub retired: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub statement: String,
    pub status: RuleStatus,
    pub severity: RuleSeverity,
    #[relation(target = Requirement, flow = target_upstream, required)]
    #[serde(
        default,
        alias = "requirementIds",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub requirement_ids: Vec<StableId>,
    #[relation(target = Resolution, flow = target_upstream)]
    #[serde(
        default,
        alias = "resolutionIds",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub resolution_ids: Vec<StableId>,
    #[serde(
        default,
        alias = "sourceDocument",
        skip_serializing_if = "Option::is_none"
    )]
    pub source_document: Option<String>,
    #[serde(
        default,
        alias = "sourceSection",
        skip_serializing_if = "Option::is_none"
    )]
    pub source_section: Option<String>,
    #[relation(none)]
    #[serde(
        default,
        alias = "originThread",
        skip_serializing_if = "Option::is_none"
    )]
    pub origin_thread: Option<StableId>,
    #[relation(none)]
    #[serde(
        default,
        alias = "originMessage",
        skip_serializing_if = "Option::is_none"
    )]
    pub origin_message: Option<StableId>,
}
