use super::fixtures::*;
use crate::wiki::model::GapKind;

#[test]
fn resolution_page_links_requirements_rules_and_spawned_work() {
    let corpus = fixture_corpus();
    let page = resolution_page(&corpus, "res_split");
    assert_eq!(page.resolves.len(), 1);
    assert_eq!(page.resolves[0].target.record_id, "req_child");
    assert_eq!(page.spawned.len(), 1);
    assert_eq!(page.spawned[0].target.record_id, "req_stuck");
    assert_eq!(page.produced_rules.len(), 1);
    assert_eq!(page.produced_rules[0].link.target.record_id, "rule_001");
    assert!(page.gaps.is_empty());
    assert_eq!(page.threads.len(), 1);
    assert_eq!(page.threads[0].thread_id, "thr_res_split");
}

#[test]
fn a_detached_decision_page_lists_nothing_and_raises_no_gap() {
    let corpus = fixture_corpus();
    let page = resolution_page(&corpus, "res_orphan");
    assert!(page.resolves.is_empty());
    assert!(page.produced_rules.is_empty());
    assert!(page.gaps.is_empty());
}

#[test]
fn rule_page_traces_back_to_requirements_and_sources() {
    let corpus = fixture_corpus();
    let page = rule_page(&corpus, "rule_001");
    assert_eq!(page.title, "Invoices grouped by participant");
    let produced_by: Vec<&str> = page
        .produced_by
        .iter()
        .map(|link| link.target.record_id.as_str())
        .collect();
    assert_eq!(produced_by, vec!["res_split", "req_child"]);
    assert_eq!(page.requirements.len(), 1);
    assert_eq!(page.requirements[0].target.record_id, "req_child");
    assert_eq!(page.sources.len(), 1);
    assert_eq!(page.sources[0].target.record_id, "source_schads");
    assert!(page.implementations.is_empty());
    assert!(page.verifications.is_empty());
    assert!(page.gaps.is_empty());
}

#[test]
fn rule_page_titles_a_nameless_rule_by_its_statement() {
    let corpus = fixture_corpus();
    let page = rule_page(&corpus, "rule_orphan");
    assert_eq!(page.title, "Claim items shall be grouped by participant");
    assert!(page.gaps.is_empty());
    assert!(page.produced_by.is_empty());
}

#[test]
fn requirement_titles_stop_at_the_first_clause_while_the_statement_stays_complete() {
    let mut state = empty_state();
    let statement = "Claims shall be grouped by participant before invoicing; later exports may preserve the original ordering for audit readers";
    state.requirements = vec![requirement(
        "req_long_title",
        statement,
        provenance_core::RequirementStatus::Active,
        vec![],
    )];

    let corpus = super::super::build_corpus(&state, &crate::wiki::links::LinkResolver::new(None));
    let page = &corpus.requirements[0];

    assert_eq!(
        page.title,
        "Claims shall be grouped by participant before invoicing"
    );
    assert_eq!(page.statement, statement);
    assert!(page.title.chars().count() <= 70);
}

#[test]
fn requirement_titles_handle_dotted_words_and_truncate_at_a_word_boundary() {
    let mut state = empty_state();
    state.requirements = vec![
        requirement(
            "req_dotnet",
            ".NET 8.0 services shall preserve participant identifiers across every invoice export",
            provenance_core::RequirementStatus::Active,
            vec![],
        ),
        requirement(
            "req_long_words",
            "Participant invoice exports shall preserve all original settlement references while remaining readable to audit reviewers",
            provenance_core::RequirementStatus::Active,
            vec![],
        ),
    ];

    let corpus = super::super::build_corpus(&state, &crate::wiki::links::LinkResolver::new(None));

    assert!(corpus.requirements[0]
        .title
        .starts_with(".NET 8.0 services"));
    assert!(corpus.requirements[1].title.ends_with('…'));
    assert!(corpus.requirements[1].title.chars().count() <= 70);
    assert!(!corpus.requirements[1].title.ends_with(" …"));

    let mut unbroken_state = empty_state();
    unbroken_state.requirements = vec![requirement(
        "req_unbroken",
        &"a".repeat(100),
        provenance_core::RequirementStatus::Active,
        vec![],
    )];
    let unbroken = super::super::build_corpus(
        &unbroken_state,
        &crate::wiki::links::LinkResolver::new(None),
    );
    assert!(unbroken.requirements[0].title.chars().count() <= 70);
    assert!(unbroken.requirements[0].title.ends_with('…'));
}

#[test]
fn source_page_lists_referencing_requirements_and_pins_links() {
    let corpus = fixture_corpus();
    let page = source_page(&corpus, "source_schads");
    assert_eq!(page.referenced_requirements.len(), 1);
    assert_eq!(
        page.referenced_requirements[0].target.record_id,
        "req_child"
    );
    assert_eq!(
        page.reference.as_ref().unwrap().href.as_deref(),
        Some("https://github.com/exampleorg/ex-api/blob/abc1234/docs/award.md")
    );
    assert!(page.gaps.is_empty());
}

#[test]
fn source_page_flags_unreferenced_sources() {
    let corpus = fixture_corpus();
    let page = source_page(&corpus, "source_unused");
    assert_eq!(gap_kinds(&page.gaps), vec![GapKind::UnreferencedSource]);
    assert!(page.referenced_requirements.is_empty());
}
