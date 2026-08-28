use super::super::*;
use crate::state_store::{
    AddSourceReferenceInput, CreateDomainInput, CreateRequirementInput, CreateResolutionInput,
    CreateRuleInput, CreateSourceInput, StateStore,
};
use provenance_core::{
    Manifest, RepoPathPrefix, RequirementStatus, ResolutionStatus, RuleSeverity, RuleStatus,
    ScopeId, SourceType, StableId,
};

pub fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

pub fn empty_layout() -> (tempfile::TempDir, ProvenanceLayout, ScopeId) {
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
    let scope = ScopeId::new("default").unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_string(&Manifest::default_with_scope(
            scope.clone(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    (dir, layout, scope)
}

pub fn seeded_layout() -> (tempfile::TempDir, ProvenanceLayout, ScopeId) {
    let (dir, layout, scope) = empty_layout();
    let store = StateStore::new(layout.clone());
    store
        .create_domain(
            None,
            CreateDomainInput {
                scope_id: scope.clone(),
                id: sid("domain_payroll"),
                name: "Payroll".into(),
                description: None,
                color: None,
            },
        )
        .unwrap();
    create_source(&store, &scope, "source_schads");
    store
        .create_requirement(
            None,
            CreateRequirementInput {
                scope_id: scope.clone(),
                id: sid("req_schads_overtime"),
                statement: "Overtime".into(),
                description: None,
                status: RequirementStatus::Active,
                domain_id: Some(sid("domain_payroll")),
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();
    attach_source(&store, &scope, "req_schads_overtime", "source_schads");
    store
        .create_resolution(
            None,
            CreateResolutionInput {
                scope_id: scope.clone(),
                id: sid("res_schads_overtime"),
                title: "Overtime interpretation".into(),
                requirement_id: Some(sid("req_schads_overtime")),
                position: "Use award threshold".into(),
                rationale: "Matches source clause".into(),
                status: ResolutionStatus::Proposed,
                context: None,
                enforcement: None,
                confidence: None,
                inputs: Vec::new(),
                made_by: None,
                approved_by: None,
                approved_at: None,
                superseded_by: None,
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();
    store
        .create_rule(
            None,
            CreateRuleInput {
                scope_id: scope.clone(),
                id: sid("rule_schads_pay_001"),
                name: None,
                description: None,
                requirement_id: Some(sid("req_schads_overtime")),
                resolution_id: Some(sid("res_schads_overtime")),
                statement: "Pay overtime after the threshold".into(),
                status: RuleStatus::Active,
                severity: RuleSeverity::High,
                source_document: None,
                source_section: None,
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();
    (dir, layout, scope)
}

pub fn create_source(store: &StateStore, scope: &ScopeId, id: &str) {
    store
        .create_source(
            None,
            CreateSourceInput {
                scope_id: scope.clone(),
                id: sid(id),
                name: id.to_string(),
                source_type: SourceType::Policy,
                url: None,
                reference: None,
                commit_pin: None,
                effective_date: None,
                review_date: None,
                superseded_by: None,
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();
}

pub fn create_requirement(
    store: &StateStore,
    scope: &ScopeId,
    id: &str,
    status: RequirementStatus,
) {
    store
        .create_requirement(
            None,
            CreateRequirementInput {
                scope_id: scope.clone(),
                id: sid(id),
                statement: format!("{id} statement"),
                description: None,
                status,
                domain_id: None,
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();
}

pub fn attach_source(store: &StateStore, scope: &ScopeId, requirement_id: &str, source_id: &str) {
    store
        .add_source_reference(
            None,
            AddSourceReferenceInput {
                scope_id: scope.clone(),
                source_id: sid(source_id),
                requirement_id: sid(requirement_id),
                clause: None,
            },
        )
        .unwrap();
}
