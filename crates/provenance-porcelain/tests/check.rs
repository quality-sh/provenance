use provenance_macros::verifies;
use provenance_porcelain::check::{
    BindingContext, BindingPolicy, Category, CategoryContext, CategoryReport, CategoryRun,
    CheckInput, CheckPort, Finding, GapsContext, PortFuture, Refusal, ScopeCounts,
    StatementContext, Status,
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
                Category::Gaps => vec![Finding::new("rule has no producing Requirement")],
                Category::Docs => vec![Finding::new("docs/guide.md links to a missing page")],
            };
            Ok(match category {
                Category::Graph => CategoryRun::Graph {
                    findings,
                    refusal: Refusal::None,
                },
                Category::Statements => CategoryRun::Statements {
                    findings,
                    context: None,
                    refusal: Refusal::None,
                },
                Category::Bindings => CategoryRun::Bindings {
                    findings,
                    context: BindingContext {
                        policy: BindingPolicy::Warning,
                    },
                    refusal: Refusal::None,
                },
                Category::Gaps => CategoryRun::Gaps {
                    findings,
                    context: GapsContext { scopes: Vec::new() },
                },
                Category::Docs => CategoryRun::Docs {
                    findings,
                    refusal: Refusal::None,
                },
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
            CategoryReport::from_run(CategoryRun::Bindings {
                findings: vec![Finding::new("active rule has no verification")],
                context: BindingContext {
                    policy: BindingPolicy::Warning,
                },
                refusal: Refusal::None,
            }),
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
    assert!(outcome.categories[0].refuses());
}

#[test]
#[verifies("rule_porcelain_check_selector_union", exhaustion)]
fn explicit_selectors_produce_their_union() {
    let categories = Category::ALL;
    for mask in 1_u8..16 {
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
#[verifies(
    "rule_porcelain_check_supplied_statement_selects_statements",
    exhaustion
)]
fn a_supplied_statement_selects_statements_and_keeps_the_other_selectors() {
    let categories = [Category::Graph, Category::Statements, Category::Bindings];
    for mask in 0_u8..8 {
        let selected = categories
            .into_iter()
            .enumerate()
            .filter_map(|(index, category)| (mask & (1 << index) != 0).then_some(category))
            .collect::<Vec<_>>();
        let input = CheckInput::new(selected.clone()).with_statement("The pump stops.");

        let mut expected = selected;
        if !expected.contains(&Category::Statements) {
            expected.push(Category::Statements);
            expected.sort_unstable();
        }
        assert_eq!(input.categories(), expected, "mask {mask}");
    }
}

/// Records the text its statement check receives and refuses the committed run.
struct StatementRecordingPort(std::sync::Mutex<Vec<(String, Option<String>)>>);

impl CheckPort for StatementRecordingPort {
    fn run<'a>(&'a self, category: Category, scope: Option<&'a str>) -> PortFuture<'a> {
        FixturePort.run(category, scope)
    }

    fn check_statement<'a>(&'a self, statement: &'a str, scope: Option<&'a str>) -> PortFuture<'a> {
        self.0
            .lock()
            .unwrap()
            .push((statement.to_owned(), scope.map(str::to_owned)));
        Box::pin(async {
            Ok(CategoryRun::Statements {
                findings: vec![Finding::new("the supplied statement has a finding")],
                context: None,
                refusal: Refusal::None,
            })
        })
    }
}

#[tokio::test]
#[verifies("rule_porcelain_check_supplied_statement_selects_statements", examples)]
async fn a_supplied_statement_replaces_the_committed_statements_in_its_category() {
    let port = StatementRecordingPort(std::sync::Mutex::new(Vec::new()));
    let service = Porcelain::new(port);

    let outcome = service
        .check(
            CheckInput::new([Category::Graph])
                .with_statement("The pump stops.")
                .in_scope("default"),
        )
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
                vec![Finding::new("the supplied statement has a finding")]
            ),
        ]
    );
    assert_eq!(
        service.into_port().0.into_inner().unwrap(),
        vec![("The pump stops.".to_owned(), Some("default".to_owned()))]
    );
}

#[tokio::test]
#[verifies("rule_porcelain_check_unavailable", examples)]
async fn a_port_without_a_statement_check_reports_statements_unavailable() {
    let outcome = Porcelain::new(FixturePort)
        .check(CheckInput::default().with_statement("The pump stops."))
        .await;

    assert_eq!(outcome.categories.len(), 1);
    assert_eq!(outcome.categories[0].category, Category::Statements);
    assert_eq!(outcome.categories[0].status, Status::Unavailable);
    assert_eq!(
        outcome.categories[0].unavailable_reason.as_deref(),
        Some("this host does not check a supplied statement")
    );
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
    let report = CategoryReport::from_run(CategoryRun::Statements {
        findings: Vec::new(),
        context: Some(StatementContext {
            candidate_commit: "abc123".into(),
            base_commit: Some("def456".into()),
        }),
        refusal: Refusal::None,
    });

    let Some(CategoryContext::Statements(context)) = report.context.as_ref() else {
        panic!("statement context")
    };
    assert_eq!(context.candidate_commit, "abc123");
    assert_eq!(report.status, Status::Passed);
}

#[test]
fn category_run_keeps_typed_binding_policy_and_refusal_together() {
    let run = CategoryRun::Bindings {
        findings: vec![Finding::new("active rule has no verification")],
        context: BindingContext {
            policy: BindingPolicy::Error,
        },
        refusal: Refusal::Findings,
    };

    let report = CategoryReport::from_run(run);

    assert!(report.refuses());
    let Some(CategoryContext::Bindings(context)) = report.context else {
        panic!("binding context")
    };
    assert_eq!(context.policy, BindingPolicy::Error);
}

/// The gaps category runs only when selected, keeps its counts with zero findings, and
/// its findings do not refuse the command.
#[tokio::test]
#[verifies("rule_check_reports_gaps", examples)]
async fn gaps_is_selected_only_and_never_refuses() {
    assert!(!CheckInput::default().categories().contains(&Category::Gaps));
    let outcome = Porcelain::new(FixturePort)
        .check(CheckInput::new([Category::Gaps]))
        .await;
    assert_eq!(outcome.categories.len(), 1);
    assert_eq!(outcome.categories[0].status, Status::Findings);
    assert!(!outcome.categories[0].refuses());

    let counts = ScopeCounts {
        scope_id: "default".into(),
        requirements: 2,
        source_linked_requirements: 1,
        resolved_requirements: 0,
        requirements_with_rules: 1,
        rules_total: 1,
        rules_with_complete_traceability: 1,
    };
    let passed = CategoryReport::from_run(CategoryRun::Gaps {
        findings: Vec::new(),
        context: GapsContext {
            scopes: vec![counts],
        },
    });
    assert_eq!(passed.status, Status::Passed);
    assert_eq!(
        serde_json::to_value(&passed).unwrap()["context"]["scopes"][0]["rules_total"],
        1
    );
}
