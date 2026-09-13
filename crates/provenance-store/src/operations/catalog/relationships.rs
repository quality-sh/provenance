//! Native relation methods enforce target kinds, required lists, and cycles.
use super::{
    actions::{native_action, RecordActionInput, ReferenceActionInput},
    ExecutionNeed, ExecutionNeeds, Operation, OperationFuture, PreparedContext,
};
use crate::{
    layout::ProvenanceLayout,
    state_store::StateStore,
    write_error::{SourceFailure, WriteError, WriteFailure},
};
native_action!(
    SetRequirementRefines,
    "set-requirement-refines",
    ReferenceActionInput,
    provenance_core::Requirement,
    |store, input| store.set_requirement_refines(&input.scope_id, &input.id, input.target_id)
);
native_action!(
    ClearRequirementRefines,
    "clear-requirement-refines",
    RecordActionInput,
    provenance_core::Requirement,
    |store, input| store.clear_requirement_refines(&input.scope_id, &input.id)
);
native_action!(
    AddRequirementDependsOn,
    "add-requirement-depends-on",
    ReferenceActionInput,
    provenance_core::Requirement,
    |store, input| store.add_requirement_depends_on(&input.scope_id, &input.id, input.target_id)
);
native_action!(
    ClearRequirementDependsOn,
    "clear-requirement-depends-on",
    ReferenceActionInput,
    provenance_core::Requirement,
    |store, input| store.clear_requirement_depends_on(&input.scope_id, &input.id, &input.target_id)
);
native_action!(
    AddRequirementSupersedes,
    "add-requirement-supersedes",
    ReferenceActionInput,
    provenance_core::Requirement,
    |store, input| store.add_requirement_supersedes(&input.scope_id, &input.id, input.target_id)
);
native_action!(
    ClearRequirementSupersedes,
    "clear-requirement-supersedes",
    ReferenceActionInput,
    provenance_core::Requirement,
    |store, input| store.clear_requirement_supersedes(&input.scope_id, &input.id, &input.target_id)
);
native_action!(
    SetRequirementSpawnedBy,
    "set-requirement-spawned-by",
    ReferenceActionInput,
    provenance_core::Requirement,
    |store, input| store.set_requirement_spawned_by(&input.scope_id, &input.id, input.target_id)
);
native_action!(
    ClearRequirementSpawnedBy,
    "clear-requirement-spawned-by",
    RecordActionInput,
    provenance_core::Requirement,
    |store, input| store.clear_requirement_spawned_by(&input.scope_id, &input.id)
);
native_action!(
    AddRuleRequirement,
    "add-rule-requirement",
    ReferenceActionInput,
    provenance_core::Rule,
    |store, input| store.add_rule_requirement(&input.scope_id, &input.id, input.target_id)
);
native_action!(
    ClearRuleRequirement,
    "clear-rule-requirement",
    ReferenceActionInput,
    provenance_core::Rule,
    |store, input| store.clear_rule_requirement(&input.scope_id, &input.id, &input.target_id)
);
native_action!(
    AddRuleResolution,
    "add-rule-resolution",
    ReferenceActionInput,
    provenance_core::Rule,
    |store, input| store.add_rule_resolution(&input.scope_id, &input.id, input.target_id)
);
native_action!(
    ClearRuleResolution,
    "clear-rule-resolution",
    ReferenceActionInput,
    provenance_core::Rule,
    |store, input| store.clear_rule_resolution(&input.scope_id, &input.id, &input.target_id)
);
native_action!(
    AddResolutionRequirement,
    "add-resolution-requirement",
    ReferenceActionInput,
    provenance_core::Resolution,
    |store, input| store.add_resolution_requirement(&input.scope_id, &input.id, input.target_id)
);
native_action!(
    ClearResolutionRequirement,
    "clear-resolution-requirement",
    ReferenceActionInput,
    provenance_core::Resolution,
    |store, input| store.clear_resolution_requirement(&input.scope_id, &input.id, &input.target_id)
);
native_action!(
    AddResolutionSupersedes,
    "add-resolution-supersedes",
    ReferenceActionInput,
    provenance_core::Resolution,
    |store, input| store.add_resolution_supersedes(&input.scope_id, &input.id, input.target_id)
);
native_action!(
    ClearResolutionSupersedes,
    "clear-resolution-supersedes",
    ReferenceActionInput,
    provenance_core::Resolution,
    |store, input| store.clear_resolution_supersedes(&input.scope_id, &input.id, &input.target_id)
);
native_action!(
    AddSourceSupersedes,
    "add-source-supersedes",
    ReferenceActionInput,
    provenance_core::Source,
    |store, input| store.add_source_supersedes(&input.scope_id, &input.id, input.target_id)
);
native_action!(
    ClearSourceSupersedes,
    "clear-source-supersedes",
    ReferenceActionInput,
    provenance_core::Source,
    |store, input| store.clear_source_supersedes(&input.scope_id, &input.id, &input.target_id)
);
native_action!(
    SetQuestionContradicts,
    "set-question-contradicts",
    ReferenceActionInput,
    provenance_core::Question,
    |store, input| store.set_question_contradicts(&input.scope_id, &input.id, input.target_id)
);
native_action!(
    ClearQuestionContradicts,
    "clear-question-contradicts",
    RecordActionInput,
    provenance_core::Question,
    |store, input| store.clear_question_contradicts(&input.scope_id, &input.id)
);
native_action!(
    ClearSourceReference,
    "clear-source-reference",
    ReferenceActionInput,
    provenance_core::Requirement,
    |store, input| store.clear_source_reference(&input.scope_id, &input.id, &input.target_id)
);
