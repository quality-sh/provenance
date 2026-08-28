use provenance_core::{
    Manifest, RepoPathPrefix, RequirementStatus, RuleSeverity, RuleStatus, ScopeId, StableId,
};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{CreateDomainInput, CreateRequirementInput, CreateRuleInput, StateStore},
};

fn seeded_store() -> (tempfile::TempDir, StateStore, ScopeId) {
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
    (dir, StateStore::new(layout), scope)
}

fn seed_domains_and_rule(store: &StateStore, scope: &ScopeId) {
    store
        .create_domain(
            None,
            CreateDomainInput {
                scope_id: scope.clone(),
                id: StableId::new("domain_payroll").unwrap(),
                name: "Payroll".into(),
                description: Some("Payroll compliance".into()),
                color: Some("#3b82f6".into()),
            },
        )
        .unwrap();
    store
        .create_domain(
            None,
            CreateDomainInput {
                scope_id: scope.clone(),
                id: StableId::new("domain_awards").unwrap(),
                name: "Awards".into(),
                description: None,
                color: None,
            },
        )
        .unwrap();
    store
        .create_requirement(
            None,
            CreateRequirementInput {
                scope_id: scope.clone(),
                id: StableId::new("req_overtime").unwrap(),
                statement: "Overtime must be traceable".into(),
                description: None,
                status: RequirementStatus::Discovery,
                domain_id: Some(StableId::new("domain_payroll").unwrap()),
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
                id: StableId::new("rule_overtime").unwrap(),
                name: None,
                description: None,
                requirement_id: Some(StableId::new("req_overtime").unwrap()),
                resolution_id: None,
                statement: "Pay overtime after threshold".into(),
                status: RuleStatus::Active,
                severity: RuleSeverity::High,
                source_document: None,
                source_section: None,
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();
}

#[test]
fn domain_records_are_written_deterministically_and_claimed_by_requirements() {
    let (_dir, store, scope) = seeded_store();

    seed_domains_and_rule(&store, &scope);

    assert_eq!(
        store.list_domains(&scope).unwrap()[0].id.as_str(),
        "domain_awards"
    );
    assert_eq!(
        store.list_requirements(&scope).unwrap()[0]
            .domain_id
            .as_ref()
            .unwrap()
            .as_str(),
        "domain_payroll"
    );
}

#[test]
fn a_second_domain_with_the_same_name_is_refused() {
    let (_dir, store, scope) = seeded_store();

    seed_domains_and_rule(&store, &scope);

    assert!(store
        .create_domain(
            None,
            CreateDomainInput {
                scope_id: scope,
                id: StableId::new("domain_payroll_again").unwrap(),
                name: "Payroll".into(),
                description: None,
                color: None,
            }
        )
        .unwrap_err()
        .to_string()
        .contains("domain name already exists"));
}
