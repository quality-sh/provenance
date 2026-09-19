use provenance_macros::verifies;
use provenance_porcelain::check::{
    binding_findings, separate_repository_findings, BindingInventory, Category, CategoryReport,
    CheckInput, CheckPort, Finding, PortFuture, RuleBindingState, Status,
};
use provenance_porcelain::Porcelain;

#[test]
#[verifies("rule_porcelain_check_defaults_all", examples)]
fn bare_check_selects_every_category() {
    let input = CheckInput::default();

    assert_eq!(
        input.categories(),
        &[Category::Graph, Category::Statements, Category::Bindings]
    );
}

struct FixturePort;

impl CheckPort for FixturePort {
    fn run(&self, category: Category) -> PortFuture<'_> {
        Box::pin(async move {
            Ok(match category {
                Category::Graph => vec![Finding::new("dangling requirement reference")],
                Category::Statements => vec![Finding::new("statement uses an unapproved term")],
                Category::Bindings => vec![Finding::new("active rule has no verification")],
            })
        })
    }
}

#[tokio::test]
#[verifies("rule_porcelain_check_categories", examples)]
async fn output_keeps_each_category_and_its_findings_separate() {
    let outcome = Porcelain::new(FixturePort)
        .check(CheckInput::default())
        .await;

    assert_eq!(
        outcome.categories,
        vec![
            CategoryReport::findings(
                Category::Graph,
                vec![Finding::new("dangling requirement reference")]
            ),
            CategoryReport::findings(
                Category::Statements,
                vec![Finding::new("statement uses an unapproved term")]
            ),
            CategoryReport::findings(
                Category::Bindings,
                vec![Finding::new("active rule has no verification")]
            ),
        ]
    );
    assert!(outcome
        .categories
        .iter()
        .all(|report| report.status == Status::Findings));
}

struct UnavailablePort;

impl CheckPort for UnavailablePort {
    fn run(&self, _: Category) -> PortFuture<'_> {
        Box::pin(async { Err("source scanner is not installed".to_owned()) })
    }
}

#[tokio::test]
#[verifies("rule_porcelain_check_unavailable", examples)]
async fn a_category_that_cannot_run_is_unavailable_not_passed() {
    let outcome = Porcelain::new(UnavailablePort)
        .check(CheckInput::new([Category::Bindings]))
        .await;

    assert_eq!(outcome.categories[0].status, Status::Unavailable);
    assert_eq!(
        outcome.categories[0].unavailable_reason.as_deref(),
        Some("source scanner is not installed")
    );
}

#[test]
#[verifies("rule_porcelain_coverage_does_not_run_tests", examples)]
fn binding_inspection_does_not_start_a_project_test_runner() {
    let directory = tempfile::tempdir().unwrap();
    let marker = directory.path().join("project-tests-ran");
    std::fs::write(
        directory.path().join("project-test.sh"),
        format!("#!/bin/sh\ntouch {}\n", marker.display()),
    )
    .unwrap();
    let inventory = BindingInventory::new([RuleBindingState::new("rule_alpha", true)]);

    let findings = binding_findings(&inventory);

    assert_eq!(findings.len(), 2);
    assert!(!marker.exists());
}

#[test]
#[verifies("rule_porcelain_missing_binding_not_invalid", examples)]
fn missing_bindings_are_coverage_findings_not_graph_findings() {
    let inventory = BindingInventory::new([RuleBindingState::new("rule_alpha", true)]);

    let findings = separate_repository_findings(Vec::new(), &inventory);

    assert!(findings.graph.is_empty());
    assert_eq!(findings.bindings.len(), 2);
}

#[test]
#[verifies("rule_porcelain_check_selector_union", exhaustion)]
fn explicit_selectors_produce_their_union() {
    let categories = [Category::Graph, Category::Statements, Category::Bindings];
    for mask in 1_u8..8 {
        let selected = categories
            .into_iter()
            .enumerate()
            .filter_map(|(index, category)| (mask & (1 << index) != 0).then_some(category))
            .collect::<Vec<_>>();
        let input = CheckInput::new(selected.clone());

        assert_eq!(input.categories(), selected);
    }
}

#[test]
#[verifies("rule_porcelain_check_categories", examples)]
fn findings_can_keep_surface_neutral_structured_details() {
    let finding = Finding::with_detail(
        "statement finding",
        serde_json::json!({"record_id": "req_alpha", "rule": "1.1"}),
    );

    assert_eq!(finding.detail.as_ref().unwrap()["record_id"], "req_alpha");
}

#[test]
#[verifies("rule_porcelain_check_categories", examples)]
fn category_reports_keep_computation_context() {
    let report = CategoryReport::with_context(
        Category::Statements,
        Vec::new(),
        serde_json::json!({"candidate_commit": "abc123", "base_commit": "def456"}),
    );

    assert_eq!(
        report.context.as_ref().unwrap()["candidate_commit"],
        "abc123"
    );
    assert_eq!(report.status, Status::Passed);
}
