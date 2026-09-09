//! Update requests use explicit clear lists so every client can preserve omissions.
use provenance_core::{
    ArtifactLink, QuestionStatus, RequirementStatus, ResolutionInput, ResolutionMethod,
    ResolutionStatus, RuleSeverity, RuleStatus, ScopeId, SourceReference, SourceType, StableId,
    TopicStatus,
};
use serde::Deserialize;

macro_rules! update_input {
    ($name:ident, $clear:ident { $($variant:ident),* }, { $($field:ident: $ty:ty),* }) => {
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $clear { $($variant),* }

        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub scope_id: ScopeId,
            pub id: StableId,
            $(pub $field: Option<$ty>,)*
            #[serde(default)]
            pub clear_fields: Vec<$clear>,
        }
    };
}

update_input!(UpdateSourceInput, SourceClearField {
    Url, Reference, CommitPin, EffectiveDate, ReviewDate
}, {
    declared_by: String, name: String, source_type: SourceType, url: String,
    reference: String, commit_pin: String, effective_date: i64, review_date: i64, retired: bool
});
update_input!(UpdateResolutionInput, ResolutionClearField {
    Context, Enforcement, Confidence, MadeBy, ApprovedBy, ApprovedAt, ReviewOn
}, {
    title: String, position: String, rationale: String, status: ResolutionStatus,
    context: String, enforcement: String, confidence: f64, inputs: Vec<ResolutionInput>,
    made_by: String, approved_by: String, approved_at: i64, review_on: String
});
update_input!(UpdateRequirementInput, RequirementClearField {
    Description, Fog, DomainId
}, {
    declared_by: String, statement: String, description: String, fog: String,
    status: RequirementStatus, domain_id: StableId, retired: bool
});
update_input!(UpdateRuleInput, RuleClearField {
    Name, Description, SourceDocument, SourceSection
}, {
    declared_by: String, name: String, description: String, statement: String,
    status: RuleStatus, severity: RuleSeverity, source_document: String,
    source_section: String, retired: bool
});
update_input!(UpdateDomainInput, DomainClearField { Description, Color }, {
    name: String, description: String, color: String
});
update_input!(UpdateBoundaryInput, BoundaryClearField { SourceRef }, {
    statement: String, source_ref: SourceReference
});
update_input!(UpdateTopicInput, TopicClearField {}, {
    title: String, status: TopicStatus, links: Vec<ArtifactLink>
});
update_input!(EditQuestionInput, QuestionClearField { ResolutionId, Contradicts }, {
    question: String, resolution_method: ResolutionMethod, status: QuestionStatus,
    links: Vec<ArtifactLink>, resolution_id: StableId, contradicts: StableId
});
