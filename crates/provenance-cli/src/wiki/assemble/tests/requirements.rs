use super::super::build_corpus;
use super::fixtures::*;
use crate::wiki::links::LinkResolver;
use crate::wiki::model::{GapKind, RecordKind};
use provenance_core::{NodeType, RequirementStatus};

#[test]
fn requirement_page_assembles_lineage_decision_rules_and_sources() {
    let corpus = fixture_corpus();
    let page = requirement_page(&corpus, "req_child");

    let back = page.back_link.as_ref().unwrap();
    assert_eq!(back.target.record_id, "req_root");

    let lineage: Vec<(&str, bool)> = page
        .lineage
        .iter()
        .map(|entry| (entry.link.target.record_id.as_str(), entry.is_current))
        .collect();
    assert_eq!(lineage, vec![("req_root", false), ("req_child", true)]);

    assert_eq!(page.decisions.len(), 1);
    let decision = &page.decisions[0];
    assert_eq!(decision.link.target.record_id, "res_split");
    assert_eq!(decision.link.target.kind, RecordKind::Resolution);
    assert_eq!(decision.position, "Adopt the split");
    assert_eq!(decision.inputs.len(), 1);
    assert!(decision.inputs[0].reference.href.is_none());

    assert_eq!(
        page.produced_rules.len(),
        1,
        "direct and via-resolution rules deduplicate"
    );
    let card = &page.produced_rules[0];
    assert_eq!(card.link.target.record_id, "rule_001");
    assert_eq!(card.evidence.len(), 1);
    assert_eq!(card.evidence[0].label, "src/UseCase.php:59-69");
    assert!(card.evidence[0].href.is_none());

    assert_eq!(page.sources.len(), 1);
    assert_eq!(page.sources[0].link.target.record_id, "source_schads");
    assert_eq!(page.sources[0].clause.as_deref(), Some("clause 10.3"));

    assert!(page.gaps.is_empty());
}

#[test]
fn requirement_page_borrows_decision_threads_without_unscanned_links() {
    let corpus = fixture_corpus();
    let page = requirement_page(&corpus, "req_child");
    let thread_ids: Vec<&str> = page
        .threads
        .iter()
        .map(|thread| thread.thread_id.as_str())
        .collect();
    assert_eq!(thread_ids, vec!["thr_req_child", "thr_res_split"]);
    assert_eq!(page.threads[1].parent_type, NodeType::Resolution);
    let note = &page.threads[1].messages[0];
    assert!(note.refs.is_empty());
}

#[test]
fn requirement_page_flags_missing_sources() {
    let corpus = fixture_corpus();
    let page = requirement_page(&corpus, "req_root");
    assert_eq!(gap_kinds(&page.gaps), vec![GapKind::MissingSourceRefs]);
    let children: Vec<&str> = page
        .children
        .iter()
        .map(|link| link.target.record_id.as_str())
        .collect();
    assert_eq!(children, vec!["req_child"]);
}

fn refining(id: &str, statement: &str, parent: &str) -> provenance_core::Requirement {
    let mut record = requirement(id, statement, RequirementStatus::Active, vec![]);
    record.refines = Some(sid(parent));
    record
}

#[test]
fn requirement_page_lists_siblings_under_the_same_parent_without_self_in_record_order() {
    let mut state = empty_state();
    state.requirements = vec![
        requirement(
            "req_parent_a",
            "Parent A",
            RequirementStatus::Active,
            vec![],
        ),
        requirement(
            "req_parent_b",
            "Parent B",
            RequirementStatus::Active,
            vec![],
        ),
        refining("req_sibling_beta", "Sibling Beta", "req_parent_a"),
        refining("req_child", "Child", "req_parent_a"),
        refining("req_sibling_alpha", "Sibling Alpha", "req_parent_a"),
        refining("req_cousin", "Cousin", "req_parent_b"),
    ];
    let resolver = LinkResolver::new(None);
    let corpus = build_corpus(&state, &resolver);
    let page = requirement_page(&corpus, "req_child");

    let sibling_ids: Vec<&str> = page
        .siblings
        .iter()
        .map(|link| link.target.record_id.as_str())
        .collect();
    assert_eq!(sibling_ids, vec!["req_sibling_beta", "req_sibling_alpha"]);
    assert!(requirement_page(&corpus, "req_parent_a")
        .siblings
        .is_empty());
}

#[test]
fn requirement_and_unfinished_pages_flag_requirements_without_domain_id_only() {
    let mut state = empty_state();
    let mut missing_domain = requirement(
        "req_missing_domain",
        "Rostering shall be assigned to a domain",
        RequirementStatus::Active,
        vec![],
    );
    missing_domain.domain_id = None;
    state.requirements = vec![
        missing_domain,
        requirement(
            "req_with_domain",
            "Payroll shall keep its domain assignment",
            RequirementStatus::Active,
            vec![],
        ),
    ];

    let resolver = LinkResolver::new(None);
    let corpus = build_corpus(&state, &resolver);
    let missing_page = requirement_page(&corpus, "req_missing_domain");
    let with_domain_page = requirement_page(&corpus, "req_with_domain");

    assert!(missing_page
        .gaps
        .iter()
        .any(|gap| gap.kind == GapKind::MissingDomainId && gap.detail.contains("no domain")));
    assert!(!with_domain_page
        .gaps
        .iter()
        .any(|gap| gap.kind == GapKind::MissingDomainId));
    assert!(corpus.unfinished.gaps.iter().any(|gap| {
        gap.kind == GapKind::MissingDomainId
            && gap
                .subject
                .as_ref()
                .is_some_and(|subject| subject.target.record_id == "req_missing_domain")
    }));
    assert!(!corpus.unfinished.gaps.iter().any(|gap| {
        gap.kind == GapKind::MissingDomainId
            && gap
                .subject
                .as_ref()
                .is_some_and(|subject| subject.target.record_id == "req_with_domain")
    }));
}

#[test]
fn requirement_page_flags_dangling_refs_and_frontier_gaps() {
    let corpus = fixture_corpus();
    let page = requirement_page(&corpus, "req_stuck");
    assert!(page.sources.is_empty());
    let kinds = gap_kinds(&page.gaps);
    assert!(kinds.contains(&GapKind::DanglingReference));
    assert!(kinds.contains(&GapKind::MissingSourceRefs));
    assert!(kinds.contains(&GapKind::NoResolvingDecision));
    assert!(kinds.contains(&GapKind::NoProducedRules));
    let dangling = page
        .gaps
        .iter()
        .find(|gap| gap.kind == GapKind::DanglingReference)
        .unwrap();
    assert_eq!(
        dangling.detail,
        "This requirement points to a source that is missing."
    );
}

#[test]
fn requirement_and_unfinished_pages_anchor_a_dangling_refinement_at_its_owner() {
    let mut state = empty_state();
    let mut surviving = requirement(
        "req_surviving",
        "Surviving requirement endpoint",
        RequirementStatus::Active,
        vec![],
    );
    surviving.refines = Some(sid("req_missing_parent"));
    surviving.depends_on = vec![sid("req_missing_dependency")];
    state.domains = vec![domain("domain_default", "Invoicing")];
    state.requirements = vec![surviving];

    let resolver = LinkResolver::new(None);
    let corpus = build_corpus(&state, &resolver);
    let page = requirement_page(&corpus, "req_surviving");

    let dangling_details: Vec<_> = page
        .gaps
        .iter()
        .filter(|gap| gap.kind == GapKind::DanglingReference)
        .map(|gap| gap.detail.as_str())
        .collect();
    assert_eq!(dangling_details.len(), 2);
    assert!(dangling_details
        .iter()
        .all(|detail| detail.contains("requirement that is missing")));
    assert!(page.back_link.is_none());
    assert_eq!(
        corpus
            .unfinished
            .gaps
            .iter()
            .filter(|gap| {
                gap.kind == GapKind::DanglingReference
                    && gap
                        .subject
                        .as_ref()
                        .is_some_and(|subject| subject.target.record_id == "req_surviving")
            })
            .count(),
        2
    );
}

#[test]
fn requirement_page_does_not_treat_a_same_id_record_of_another_kind_as_a_resolving_decision() {
    // A Resolution and a Source share the stable id "dup_id". The
    // requirement cites the source; only a resolution's own
    // `requirement_ids` makes a decision, so the citation must not be
    // mistaken for one just because the ids match.
    let mut state = empty_state();
    state.requirements = vec![requirement(
        "req_child",
        "SaveInvoice shall split claim items",
        RequirementStatus::Active,
        vec![provenance_core::SourceReference {
            source_id: sid("dup_id"),
            clause: None,
        }],
    )];
    state.resolutions = vec![resolution("dup_id", "Decoy resolution", vec![])];
    state.sources = vec![source("dup_id", "Decoy source")];
    let resolver = LinkResolver::new(None);
    let corpus = build_corpus(&state, &resolver);
    let page = requirement_page(&corpus, "req_child");
    assert!(
        page.decisions.is_empty(),
        "resolution 'dup_id' names no requirement and must not appear as a decision"
    );
    assert_eq!(page.sources.len(), 1);
}

#[test]
fn contradiction_gap_surfaces_on_both_requirement_pages_without_duplicate_frontier_item() {
    let mut state = empty_state();
    state.requirements = vec![
        requirement(
            "req_left",
            "Platform shall prefer the left branch",
            RequirementStatus::Active,
            vec![],
        ),
        requirement(
            "req_right",
            "Platform shall prefer the right branch",
            RequirementStatus::Active,
            vec![],
        ),
    ];
    state.topics = vec![topic(
        "topic_branch",
        "req_left",
        provenance_core::TopicStatus::Explored,
    )];
    let mut contradiction = question(
        "question_branch",
        "topic_branch",
        "req_left",
        provenance_core::QuestionStatus::Open,
    );
    contradiction.contradicts = Some(sid("req_right"));
    state.questions = vec![contradiction];
    let resolver = LinkResolver::new(None);
    let corpus = build_corpus(&state, &resolver);

    let left_page = requirement_page(&corpus, "req_left");
    let left_contradictions: Vec<_> = left_page
        .gaps
        .iter()
        .filter(|gap| gap.kind == GapKind::UnresolvedContradictsPair)
        .collect();
    assert_eq!(left_contradictions.len(), 1);
    assert_eq!(
        left_contradictions[0]
            .related
            .as_ref()
            .unwrap()
            .target
            .record_id,
        "req_right"
    );

    let right_page = requirement_page(&corpus, "req_right");
    let right_contradictions: Vec<_> = right_page
        .gaps
        .iter()
        .filter(|gap| gap.kind == GapKind::UnresolvedContradictsPair)
        .collect();
    assert_eq!(right_contradictions.len(), 1);
    assert_eq!(
        right_contradictions[0]
            .related
            .as_ref()
            .unwrap()
            .target
            .record_id,
        "req_left"
    );

    let pair_count = compute_state_gaps(&state)
        .iter()
        .filter(|gap| gap.kind == GapKind::UnresolvedContradictsPair)
        .count();
    assert_eq!(pair_count, 1);
}
