use super::Store;
use provenance_core::{
    Manifest, RepoPathPrefix, RequirementStatus, RuleSeverity, RuleStatus, ScopeId, SourceType,
    StableId,
};
use provenance_store::repository::Repository;
use provenance_store::state_store::{CreateRequirementInput, CreateRuleInput, CreateSourceInput};

fn initialized_store() -> (tempfile::TempDir, Store, ScopeId) {
    let directory = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let layout = provenance_store::layout::ProvenanceLayout::new(root.clone());
    let scope = ScopeId::new("default").unwrap();
    std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&Manifest::default_with_scope(
            scope.clone(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    let repository = Repository::open(&root, "default").unwrap();
    (directory, Store::new(&repository), scope)
}

fn seeded_store() -> (tempfile::TempDir, Store, ScopeId) {
    let (directory, store, scope) = initialized_store();
    store
        .create_source(CreateSourceInput {
            scope_id: scope.clone(),
            id: StableId::new("source_policy").unwrap(),
            name: "Policy".into(),
            source_type: SourceType::Policy,
            url: None,
            reference: None,
            commit_pin: None,
            supersedes: Vec::new(),
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: StableId::new("req_policy").unwrap(),
            statement: "The system follows the policy".into(),
            description: None,
            status: RequirementStatus::Active,
            refines: Vec::new(),
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_rule(CreateRuleInput {
            scope_id: scope.clone(),
            id: StableId::new("rule_policy").unwrap(),
            name: None,
            description: None,
            requirement_ids: vec![StableId::new("req_policy").unwrap()],
            resolution_ids: Vec::new(),
            statement: "The service applies the policy".into(),
            status: RuleStatus::Active,
            severity: RuleSeverity::High,
            archived_in_commit: None,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    (directory, store, scope)
}

#[test]
fn the_snapshot_reads_the_scope_the_repository_selected() {
    let (_directory, store, scope) = seeded_store();

    assert_eq!(store.repository().scope(), &scope);
    assert_eq!(store.snapshot().unwrap().rules.len(), 1);
}
