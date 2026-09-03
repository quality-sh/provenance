use super::super::build_corpus;
use super::fixtures::{domain, empty_state, requirement, resolution, rule, sid, source};
use crate::wiki::links::LinkResolver;
use crate::wiki::model::DomainState;
use crate::wiki::render::render_corpus;
use provenance_core::RequirementStatus;
use provenance_macros::verifies;

#[test]
#[verifies("rule_wiki_homepage_search_coverage", examples)]
fn discovery_indexes_every_reader_facing_record_kind() {
    let mut state = empty_state();
    state.requirements = vec![requirement(
        "req_invoice",
        "Invoices shall identify the participant",
        RequirementStatus::Active,
        vec![],
    )];
    state.rules = vec![rule("rule_invoice", Some("Group invoices"))];
    state.resolutions = vec![resolution("res_invoice", "Choose invoice grouping", vec![])];
    state.sources = vec![source("source_award", "Invoice award")];

    let corpus = build_corpus(&state, &LinkResolver::new(None));

    assert_eq!(corpus.search.entries.len(), 4);
    assert_eq!(
        corpus.search.entries[0].link.title,
        "Invoices shall identify the participant"
    );
    assert_eq!(
        corpus.search.entries[0].statement,
        "Invoices shall identify the participant"
    );
    assert_eq!(
        corpus.search.entries[1].link.title,
        "Choose invoice grouping"
    );
    assert_eq!(corpus.search.entries[1].statement, "Adopt the split");
    assert_eq!(corpus.search.entries[2].link.title, "Group invoices");
    assert_eq!(
        corpus.search.entries[2].statement,
        "Claim items shall be grouped by participant"
    );
    assert_eq!(corpus.search.entries[3].link.title, "Invoice award");
    assert_eq!(
        corpus.search.coverage,
        "Search covers requirements, decisions, rules, and sources."
    );
    assert_eq!(
        corpus.index.search_coverage,
        "Search covers requirements, decisions, rules, and sources."
    );

    let decisions = render_corpus(&corpus)
        .into_iter()
        .find(|page| page.route == "/decisions/")
        .expect("the decisions index must be rendered");
    assert!(decisions.html.contains("Choose invoice grouping"));
    assert!(decisions.html.contains("Adopt the split"));
    assert!(decisions
        .html
        .contains("href=\"/resolutions/res_invoice/\""));
}

#[test]
fn rule_display_names_use_title_then_statement_clause_then_desnaked_id() {
    let mut state = empty_state();
    let mut explicit_rule = rule("rule_titled", Some("  Titled rule  "));
    explicit_rule.statement = "Ignored statement".to_string();
    let mut stated = rule("rule_stated", None);
    stated.statement = "First clause; second clause.".to_string();
    let mut desnaked = rule("rule_invoice_retry_policy", Some("  "));
    desnaked.statement = "  ".to_string();
    let mut dotted = rule("rule_dotted", None);
    dotted.statement = "Apply 1.5 times the rate. Keep the result.".to_string();
    state.rules = vec![explicit_rule, stated, desnaked, dotted];

    let corpus = build_corpus(&state, &LinkResolver::new(None));
    let page_titles = corpus
        .rules
        .iter()
        .map(|page| page.title.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        page_titles,
        vec![
            "Titled rule",
            "First clause",
            "Rule invoice retry policy",
            "Apply 1.5 times the rate",
        ]
    );
    assert_eq!(corpus.search.entries[0].link.title, "Titled rule");
    assert_eq!(corpus.search.entries[1].link.title, "First clause");
    assert_eq!(
        corpus.search.entries[2].link.title,
        "Rule invoice retry policy"
    );
}

#[test]
fn homepage_and_search_use_a_real_indexed_title_as_the_search_example() {
    let mut state = empty_state();
    state.requirements = vec![requirement(
        "req_invoice",
        "Invoices shall identify the participant",
        RequirementStatus::Active,
        vec![],
    )];
    let corpus = build_corpus(&state, &LinkResolver::new(None));

    let homepage = crate::wiki::render::render_index("default", &corpus.index);
    let search = crate::wiki::render::render_search("default", &corpus.search);

    for html in [homepage, search] {
        assert!(
            html.contains("placeholder=\"e.g. Invoices shall identify the participant\""),
            "{html}"
        );
        assert!(!html.contains("invoice participant"), "{html}");
    }
}

#[test]
fn search_example_skips_truncated_index_titles() {
    let mut state = empty_state();
    state.requirements = vec![requirement(
        "req_verbose",
        "This deliberately overlong requirement title keeps going until the indexed title has to be truncated at a word boundary",
        RequirementStatus::Active,
        vec![],
    )];
    state.rules = vec![rule("rule_invoice", Some("Group invoices"))];

    let corpus = build_corpus(&state, &LinkResolver::new(None));

    assert!(corpus.search.entries[0].link.title.ends_with('…'));
    assert_eq!(corpus.search.example.as_deref(), Some("Group invoices"));
    assert!(!corpus.search.example.as_deref().unwrap().ends_with('…'));
}

#[test]
#[verifies("rule_domain_attribution", examples)]
fn domains_group_rules_through_canonical_requirement_relationships() {
    let mut state = empty_state();
    state.domains = vec![domain("domain_default", "Invoicing")];
    state.requirements = vec![requirement(
        "req_invoice",
        "Invoices shall identify the participant",
        RequirementStatus::Active,
        vec![],
    )];
    let mut invoice_rule = rule("rule_invoice", Some("Group invoices"));
    invoice_rule.requirement_ids = vec![sid("req_invoice")];
    state.rules = vec![invoice_rule];

    let corpus = build_corpus(&state, &LinkResolver::new(None));
    let group = &corpus.domains.groups[0];

    assert!(matches!(&group.state, DomainState::Defined { id, .. } if id == "domain_default"));
    assert_eq!(group.requirements[0].target.record_id, "req_invoice");
    assert_eq!(group.rules[0].target.record_id, "rule_invoice");
}

#[test]
#[verifies("rule_domain_attribution", examples)]
fn domains_group_children_and_rules_by_their_root_requirement_domain() {
    let mut state = empty_state();
    state.domains = vec![domain("domain_default", "Invoicing")];
    let root = requirement(
        "req_root",
        "Invoices shall identify the participant",
        RequirementStatus::Active,
        vec![],
    );
    let mut child = requirement(
        "req_child",
        "Invoice lines shall identify the participant",
        RequirementStatus::Active,
        vec![],
    );
    child.domain_id = None;
    child.refines = Some(sid("req_root"));
    state.requirements = vec![root, child];
    let mut invoice_rule = rule("rule_invoice", Some("Group invoices"));
    invoice_rule.requirement_ids = vec![sid("req_child")];
    state.rules = vec![invoice_rule];

    let corpus = build_corpus(&state, &LinkResolver::new(None));
    let group = &corpus.domains.groups[0];

    assert_eq!(corpus.domains.groups.len(), 1);
    assert_eq!(
        group
            .requirements
            .iter()
            .map(|link| link.target.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["req_root", "req_child"]
    );
    assert_eq!(group.rules[0].target.record_id, "rule_invoice");
}

#[test]
#[verifies("rule_domain_attribution", examples)]
fn domains_surface_defined_missing_and_unassigned_without_dropping_rules() {
    let mut state = empty_state();
    state.domains = vec![domain("domain_default", "Invoicing")];
    let defined = requirement("req_defined", "Defined", RequirementStatus::Active, vec![]);
    let mut missing = requirement("req_missing", "Missing", RequirementStatus::Active, vec![]);
    missing.domain_id = Some(sid("domain_missing"));
    let mut unassigned = requirement(
        "req_unassigned",
        "Unassigned",
        RequirementStatus::Active,
        vec![],
    );
    unassigned.domain_id = None;
    state.requirements = vec![defined, missing, unassigned];
    let mut missing_rule = rule("rule_missing", Some("Missing rule"));
    missing_rule.requirement_ids = vec![sid("req_missing")];
    let mut unassigned_rule = rule("rule_unassigned", Some("Unassigned rule"));
    unassigned_rule.requirement_ids = vec![sid("req_unassigned")];
    state.rules = vec![missing_rule, unassigned_rule];

    let corpus = build_corpus(&state, &LinkResolver::new(None));

    assert_eq!(corpus.domains.groups.len(), 3);
    assert!(matches!(
        corpus.domains.groups[0].state,
        DomainState::Defined { .. }
    ));
    assert!(matches!(
        &corpus.domains.groups[1].state,
        DomainState::Missing { id } if id == "domain_missing"
    ));
    assert!(matches!(
        corpus.domains.groups[2].state,
        DomainState::Unassigned
    ));
    assert_eq!(
        corpus.domains.groups[1].rules[0].target.record_id,
        "rule_missing"
    );
    assert_eq!(
        corpus.domains.groups[2].rules[0].target.record_id,
        "rule_unassigned"
    );
}

#[test]
fn domains_without_authored_groups_render_flat_records_with_statements() {
    let mut state = empty_state();
    let mut requirement = requirement(
        "req_invoice",
        "Invoices shall identify the participant",
        RequirementStatus::Active,
        vec![],
    );
    requirement.domain_id = None;
    state.requirements = vec![requirement];
    state.rules = vec![rule("rule_invoice", Some("Group invoices"))];

    let corpus = build_corpus(&state, &LinkResolver::new(None));
    let html = crate::wiki::render::render_domains("default", &corpus.domains);

    assert!(html.contains("No domains have been authored"), "{html}");
    assert!(html.contains(">All requirements</h2>"), "{html}");
    assert!(html.contains(">All rules</h2>"), "{html}");
    assert!(
        html.contains("Invoices shall identify the participant"),
        "{html}"
    );
    assert!(
        html.contains("Claim items shall be grouped by participant"),
        "{html}"
    );
    assert!(!html.contains(">Unassigned</h2>"), "{html}");
    assert!(html.contains("0 groups"), "{html}");
    assert!(
        !html.contains("Rules inherit every Domain represented by their upstream requirements."),
        "{html}"
    );
}

#[test]
fn missing_domain_classification_does_not_link_to_an_absent_group() {
    let mut state = empty_state();
    let mut requirement = requirement(
        "req_invoice",
        "Invoices shall identify the participant",
        RequirementStatus::Active,
        vec![],
    );
    requirement.domain_id = Some(sid("domain_missing"));
    state.requirements = vec![requirement];

    let corpus = build_corpus(&state, &LinkResolver::new(None));
    let requirement = crate::wiki::render::render_requirement("default", &corpus.requirements[0]);

    assert!(
        requirement.contains(">domain_missing</span>"),
        "{requirement}"
    );
    assert!(
        !requirement.contains("href=\"/domains/#domain-domain_missing\""),
        "{requirement}"
    );
}

#[test]
fn missing_domain_classification_links_when_the_gap_group_is_rendered() {
    let mut state = empty_state();
    state.domains = vec![domain("domain_authored", "Authored")];
    let mut requirement = requirement(
        "req_invoice",
        "Invoices shall identify the participant",
        RequirementStatus::Active,
        vec![],
    );
    requirement.domain_id = Some(sid("domain_missing"));
    state.requirements = vec![requirement];

    let corpus = build_corpus(&state, &LinkResolver::new(None));
    let requirement = crate::wiki::render::render_requirement("default", &corpus.requirements[0]);
    let domains = crate::wiki::render::render_domains("default", &corpus.domains);

    assert!(
        requirement.contains("href=\"/domains/#domain-domain_missing\""),
        "{requirement}"
    );
    assert!(
        domains.contains("id=\"domain-domain_missing\""),
        "{domains}"
    );
}

#[test]
fn empty_scope_still_has_discovery_pages() {
    let corpus = build_corpus(&empty_state(), &LinkResolver::new(None));
    assert!(corpus.domains.groups.is_empty());
    assert!(corpus.search.entries.is_empty());
}
