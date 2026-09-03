use super::super::build_corpus;
use super::fixtures::*;
use crate::wiki::links::LinkResolver;
use crate::wiki::model::{CorpusCounts, GapKind};
use crate::wiki::render::render_corpus;
use provenance_core::{NodeType, QuestionStatus, RequirementStatus, TopicStatus};

#[test]
fn build_corpus_on_a_truly_empty_scope_is_honestly_empty() {
    let resolver = LinkResolver::new(None);
    let corpus = build_corpus(&empty_state(), &resolver);
    assert!(corpus.requirements.is_empty());
    assert!(corpus.resolutions.is_empty());
    assert!(corpus.rules.is_empty());
    assert!(corpus.sources.is_empty());
    assert_eq!(corpus.index.counts, CorpusCounts::default());
    assert_eq!(corpus.index.title, "Default documentation");
}

#[test]
fn index_reports_record_counts() {
    let corpus = fixture_corpus();
    assert_eq!(corpus.index.counts.sources, 2);
    assert_eq!(corpus.index.counts.requirements, 3);
    assert_eq!(corpus.index.counts.resolutions, 2);
    assert_eq!(corpus.index.counts.rules, 2);
}

#[test]
fn unfinished_reports_a_gap_for_a_thread_whose_parent_record_is_gone() {
    // A thread whose parent has been deleted/renamed is never matched
    // by any page's threads_for() lookup (those only ever query ids of
    // records that were found), so it would otherwise be dropped
    // without a trace instead of becoming a gap notice like every
    // other kind of dangling reference.
    let mut state = empty_state();
    state.domains = vec![domain("domain_default", "Invoicing")];
    state.requirements = vec![requirement(
        "req_child",
        "SaveInvoice shall split claim items",
        RequirementStatus::Active,
        vec![],
    )];
    state.threads = vec![thread(
        "thr_ghost",
        (NodeType::Resolution, "res_missing"),
        10,
    )];
    let resolver = LinkResolver::new(None);
    let corpus = build_corpus(&state, &resolver);
    let dangling = corpus
        .unfinished
        .gaps
        .iter()
        .find(|gap| gap.kind == GapKind::DanglingReference)
        .expect("a dangling thread parent should be reported as a gap");
    assert_eq!(
        dangling.detail,
        "A discussion belongs to a decision that is missing."
    );
    assert!(dangling.subject.is_none());
}

#[test]
fn unfinished_count_preserves_every_computed_gap_exactly_once() {
    let state = fixture_state();
    let expected = compute_state_gaps(&state)
        .into_iter()
        .map(|gap| gap.kind)
        .collect::<Vec<_>>();
    let corpus = build_corpus(&state, &LinkResolver::new(None));
    assert_eq!(corpus.unfinished.item_count(), expected.len());
    assert_eq!(corpus.index.unfinished_count, expected.len());
}

#[test]
fn unfinished_page_renders_gaps_orphans_and_open_questions() {
    let mut state = fixture_state();
    state.topics = vec![topic("topic_open", "req_stuck", TopicStatus::Open)];
    state.questions = vec![question(
        "question_open",
        "topic_open",
        "req_stuck",
        QuestionStatus::Open,
    )];
    let corpus = build_corpus(&state, &LinkResolver::new(None));

    let unfinished = render_corpus(&corpus)
        .into_iter()
        .find(|page| page.route == "/unfinished/")
        .expect("Unfinished must be the one aggregate page");

    for heading in ["Gaps", "Orphans", "Open questions"] {
        assert!(
            unfinished.html.contains(heading),
            "missing heading {heading}"
        );
    }
    assert!(unfinished.html.contains("citation gap"));
    assert!(unfinished.html.contains("Unused API spec"));
    assert!(unfinished.html.contains("What remains unresolved?"));
    assert_eq!(
        corpus.unfinished.orphans.sources[0].reason,
        "no requirement references this source"
    );
    assert!(!render_corpus(&corpus)
        .iter()
        .any(|page| page.route == "/findings/"));
}
