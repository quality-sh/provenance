//! Edits use the same scoped writer adapter as creation.
use super::creation::creation;
use super::ExecutionNeed;
use crate::state_store::{
    CreateQuestionInput, CreateTopicInput, EditQuestionInput, UpdateRequirementInput,
    UpdateResolutionInput, UpdateRuleInput, UpdateSourceInput, UpdateTopicInput,
};
creation!(
    UpdateSource,
    "update-source",
    UpdateSourceInput,
    provenance_core::Source,
    update_source,
    [GraphStorage]
);
creation!(
    UpdateResolution,
    "update-resolution",
    UpdateResolutionInput,
    provenance_core::Resolution,
    update_resolution,
    [GraphStorage]
);
creation!(
    UpdateRequirementNative,
    "update-requirement-native",
    UpdateRequirementInput,
    provenance_core::Requirement,
    update_requirement,
    [GraphStorage, Dictionary]
);
creation!(
    UpdateRule,
    "update-rule",
    UpdateRuleInput,
    provenance_core::Rule,
    update_rule,
    [GraphStorage, Dictionary]
);
creation!(
    UpdateTopic,
    "update-topic",
    UpdateTopicInput,
    provenance_core::Topic,
    edit_topic,
    [GraphStorage]
);
creation!(
    UpdateQuestion,
    "update-question",
    EditQuestionInput,
    provenance_core::Question,
    edit_question,
    [GraphStorage]
);
creation!(
    CreateTopic,
    "create-topic",
    CreateTopicInput,
    provenance_core::Topic,
    create_topic,
    [GraphStorage]
);
creation!(
    CreateQuestion,
    "create-question",
    CreateQuestionInput,
    provenance_core::Question,
    create_question,
    [GraphStorage]
);
