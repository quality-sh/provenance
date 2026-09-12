use super::{NotFound, Store};
use provenance_core::{
    Manifest, RepoPathPrefix, RequirementStatus, RuleSeverity, RuleStatus, ScopeId, SourceType,
    StableId,
};
use provenance_store::state_store::{CreateRequirementInput, CreateRuleInput, CreateSourceInput};

fn initialized_store() -> (tempfile::TempDir, Store, ScopeId) {
    let directory = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let store = Store::open(root);
    let scope = ScopeId::new("default").unwrap();
    std::fs::create_dir_all(store.layout().manifest_path().parent().unwrap()).unwrap();
    std::fs::write(
        store.layout().manifest_path(),
        serde_json::to_vec(&Manifest::default_with_scope(
            scope.clone(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    (directory, store, scope)
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
            effective_date: None,
            review_date: None,
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
            domain_id: None,
            refines: None,
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
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    (directory, store, scope)
}

#[test]
fn open_builds_the_layout_from_the_repository_root() {
    let directory = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();

    let store = Store::open(root.clone());

    assert_eq!(store.layout().state_dir(), root.join(".provenance/state"));
}

#[test]
fn point_reads_return_each_requested_record() {
    let (_directory, store, scope) = seeded_store();

    assert_eq!(
        store
            .source(&scope, &StableId::new("source_policy").unwrap())
            .unwrap()
            .id
            .as_str(),
        "source_policy"
    );
    assert_eq!(
        store
            .requirement(&scope, &StableId::new("req_policy").unwrap())
            .unwrap()
            .id
            .as_str(),
        "req_policy"
    );
    assert_eq!(
        store
            .rule(&scope, &StableId::new("rule_policy").unwrap())
            .unwrap()
            .id
            .as_str(),
        "rule_policy"
    );
}

#[test]
fn point_read_refusals_keep_the_command_messages() {
    let (_directory, store, scope) = initialized_store();
    let missing_requirement = StableId::new("req_missing").unwrap();
    let missing_rule = StableId::new("rule_missing").unwrap();
    let missing_source = StableId::new("source_missing").unwrap();

    assert_not_found(
        store.requirement(&scope, &missing_requirement),
        &NotFound::Requirement,
        "requirement does not exist",
    );
    assert_not_found(
        store.rule(&scope, &missing_rule),
        &NotFound::Rule(missing_rule),
        "rule `rule_missing` not found in scope",
    );
    assert_not_found(
        store.source(&scope, &missing_source),
        &NotFound::Source,
        "source does not exist",
    );
}

fn assert_not_found<T: std::fmt::Debug>(
    result: anyhow::Result<T>,
    expected: &NotFound,
    message: &str,
) {
    let error = result.unwrap_err();
    assert_eq!(error.to_string(), message);
    assert_eq!(error.downcast_ref::<NotFound>(), Some(expected));
}

#[test]
fn graph_records_borrows_the_graph_families_from_the_full_snapshot() {
    let (_directory, store, scope) = seeded_store();

    let snapshot = store.snapshot(&scope).unwrap();
    let graph = snapshot.graph_records();

    assert_eq!(graph.sources[0].id.as_str(), "source_policy");
    assert_eq!(graph.requirements[0].id.as_str(), "req_policy");
    assert_eq!(graph.rules[0].id.as_str(), "rule_policy");
    assert!(snapshot.threads.is_empty());
    assert!(snapshot.proposal_cards.is_empty());
}
