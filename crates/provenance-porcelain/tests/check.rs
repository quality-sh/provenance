use provenance_macros::verifies;
use provenance_porcelain::check::{
    BindingContext, BindingPolicy, Category, CategoryContext, CategoryReport, CategoryRun,
    CheckInput, CheckPort, Finding, PortFuture, Refusal, StatementContext, Status,
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
    fn run<'a>(&'a self, category: Category, _: Option<&'a str>) -> PortFuture<'a> {
        Box::pin(async move {
            let findings = match category {
                Category::Graph => vec![Finding::new("dangling requirement reference")],
                Category::Statements => vec![Finding::new("statement uses an unapproved term")],
                Category::Bindings => vec![Finding::new("active rule has no verification")],
            };
            Ok(CategoryRun::new(category, findings))
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
    fn run<'a>(&'a self, _: Category, _: Option<&'a str>) -> PortFuture<'a> {
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
    let report = CategoryReport::from_run(CategoryRun::new(
        Category::Statements,
        Vec::new(),
    ).with_context(CategoryContext::Statements(StatementContext {
        candidate_commit: "abc123".into(),
        base_commit: Some("def456".into()),
    })));

    let Some(CategoryContext::Statements(context)) = report.context.as_ref() else {
        panic!("statement context")
    };
    assert_eq!(context.candidate_commit, "abc123");
    assert_eq!(report.status, Status::Passed);
}

#[test]
fn category_run_keeps_typed_binding_policy_and_refusal_together() {
    let run = CategoryRun::new(
        Category::Bindings,
        vec![Finding::new("active rule has no verification")],
    )
    .with_context(CategoryContext::Bindings(BindingContext {
        policy: BindingPolicy::Error,
    }))
    .with_refusal(Refusal::Findings);

    let report = CategoryReport::from_run(run);

    assert!(report.refuses());
    let Some(CategoryContext::Bindings(context)) = report.context else {
        panic!("binding context")
    };
    assert_eq!(context.policy, BindingPolicy::Error);
}
