//! Update requests use explicit clear lists so every client can preserve omissions.
use crate::review::ListEdit;
use provenance_core::{
    ArtifactLink, QuestionStatus, RequirementStatus, ResolutionInput, ResolutionMethod,
    ResolutionStatus, RuleSeverity, RuleStatus, ScopeId, SourceType, StableId, TopicStatus,
};
use provenance_macros::{rule, verifies};
use serde::{Deserialize, Serialize};

macro_rules! update_input {
    // A record type with one or more fields that an update can clear.
    (
        $(#[$attribute:meta])*
        struct $name:ident $fields:tt
        clear $clear:ident { $($variant:ident),+ }
    ) => {
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $clear { $($variant),+ }

        update_input!(@request $(#[$attribute])* $name, $fields, {
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub clear_fields: Vec<$clear>,
        });
    };
    // A record type with no field that an update can clear. Its request has no clear list.
    ($(#[$attribute:meta])* struct $name:ident $fields:tt) => {
        update_input!(@request $(#[$attribute])* $name, $fields, {});
    };
    (
        @request $(#[$attribute:meta])* $name:ident,
        { $($field:ident: $ty:ty),* },
        { $($clear_list:tt)* }
    ) => {
        $(#[$attribute])*
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[derive(Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub scope_id: ScopeId,
            pub id: StableId,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub expected_etag: Option<String>,
            $(pub $field: Option<$ty>,)*
            $($clear_list)*
        }
    };
}

update_input!(
    struct UpdateSourceInput {
        declared_by: String, name: String, source_type: SourceType, url: String,
        reference: String, commit_pin: String,
        supersedes: ListEdit
    }
    clear SourceClearField { Url, Reference, CommitPin }
);
update_input!(
    struct UpdateResolutionInput {
        title: String, position: String, rationale: String, status: ResolutionStatus,
        context: String, inputs: Vec<ResolutionInput>,
        made_by: String, approved_by: String, approved_at: i64,
        archived_in_commit: provenance_core::ArchivedStamp,
        requirement_ids: ListEdit, supersedes: ListEdit
    }
    clear ResolutionClearField { Context, MadeBy, ApprovedBy, ApprovedAt }
);
update_input!(
    struct UpdateRequirementInput {
        declared_by: String, statement: String, description: String,
        status: RequirementStatus, archived_in_commit: provenance_core::ArchivedStamp
    }
    clear RequirementClearField { Description }
);
update_input!(
    struct UpdateRuleInput {
        declared_by: String, name: String, description: String, statement: String,
        status: RuleStatus, severity: RuleSeverity, source_document: String,
        source_section: String, archived_in_commit: provenance_core::ArchivedStamp,
        requirement_ids: ListEdit, resolution_ids: ListEdit
    }
    clear RuleClearField { Name, Description, SourceDocument, SourceSection }
);
update_input!(
    /// A Topic has no field that an update can clear, so its update request has no clear
    /// list. The request refuses each field that it does not declare.
    #[rule("rule_update_clear_list_only_for_clearable_fields")]
    #[verifies("rule_update_clear_list_only_for_clearable_fields", construction)]
    struct UpdateTopicInput {
        title: String, status: TopicStatus, links: Vec<ArtifactLink>,
        archived_in_commit: provenance_core::ArchivedStamp
    }
);
update_input!(
    struct EditQuestionInput {
        question: String, resolution_method: ResolutionMethod, status: QuestionStatus,
        depends_on: Vec<StableId>,
        links: Vec<ArtifactLink>, resolution_id: StableId, contradicts: StableId,
        archived_in_commit: provenance_core::ArchivedStamp
    }
    clear QuestionClearField { ResolutionId, Contradicts }
);
