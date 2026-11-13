//! Update requests use explicit clear lists so every client can preserve omissions.
use crate::review::ListEdit;
use provenance_core::{
    ArtifactLink, QuestionStatus, RequirementStatus, ResolutionInput, ResolutionMethod,
    ResolutionStatus, RuleSeverity, RuleStatus, ScopeId, SourceType, StableId, TopicStatus,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

macro_rules! update_input {
    ($name:ident, $clear:ident { $($variant:ident),* }, $fields:tt) => {
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $clear { $($variant),* }

        update_input!($name, $clear, $fields);
    };
    ($name:ident, $clear:ident, { $($field:ident: $ty:ty),* }) => {
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[derive(Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub scope_id: ScopeId,
            pub id: StableId,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub expected_etag: Option<String>,
            $(pub $field: Option<$ty>,)*
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub clear_fields: Vec<$clear>,
        }
    };
}

update_input!(UpdateSourceInput, SourceClearField {
    Url, Reference, CommitPin
}, {
    declared_by: String, name: String, source_type: SourceType, url: String,
    reference: String, commit_pin: String,
    supersedes: ListEdit
});
update_input!(UpdateResolutionInput, ResolutionClearField {
    Context, MadeBy, ApprovedBy, ApprovedAt
}, {
    title: String, position: String, rationale: String, status: ResolutionStatus,
    context: String, inputs: Vec<ResolutionInput>,
    made_by: String, approved_by: String, approved_at: i64,
    archived_in_commit: provenance_core::ArchivedStamp,
    requirement_ids: ListEdit, supersedes: ListEdit
});
update_input!(UpdateRequirementInput, RequirementClearField {
    Description
}, {
    declared_by: String, statement: String, description: String,
    status: RequirementStatus, archived_in_commit: provenance_core::ArchivedStamp
});
update_input!(UpdateRuleInput, RuleClearField {
    Name, Description, SourceDocument, SourceSection
}, {
    declared_by: String, name: String, description: String, statement: String,
    status: RuleStatus, severity: RuleSeverity, source_document: String,
    source_section: String, archived_in_commit: provenance_core::ArchivedStamp,
    requirement_ids: ListEdit, resolution_ids: ListEdit
});
update_input!(UpdateTopicInput, TopicClearField, {
    title: String, status: TopicStatus, links: Vec<ArtifactLink>,
    archived_in_commit: provenance_core::ArchivedStamp
});

/// A Topic has no field that an update clears, so this type has no value, and
/// an update of a Topic takes only an empty list of fields to clear.
///
/// It is a struct and not an enum with no variants, because the `empty_enums`
/// lint refuses that enum. Its impls give the serialized form, the errors and
/// the schema of such an enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopicClearField(pub Infallible);

impl Serialize for TopicClearField {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        match self.0 {}
    }
}

impl<'de> Deserialize<'de> for TopicClearField {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        provenance_core::protocol::empty::deserialize_enum(deserializer, "TopicClearField")
            .map(Self)
    }
}

#[cfg(feature = "schema")]
impl schemars::JsonSchema for TopicClearField {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "TopicClearField".into()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        concat!(module_path!(), "::TopicClearField").into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::Schema::from(false)
    }
}
update_input!(EditQuestionInput, QuestionClearField { ResolutionId, Contradicts }, {
    question: String, resolution_method: ResolutionMethod, status: QuestionStatus,
    depends_on: Vec<StableId>,
    links: Vec<ArtifactLink>, resolution_id: StableId, contradicts: StableId,
    archived_in_commit: provenance_core::ArchivedStamp
});
