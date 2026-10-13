use super::super::build_corpus;
use crate::wiki::fixtures_scale::pr_45_scale_state;
use crate::wiki::links::LinkResolver;

/// The PR #45 homepage fixture, assembled end to end. The corpus is the
/// scale surface for the shared graph index: every page read below is an
/// indexed lookup, and the totals are exact for the generated relations.
#[test]
fn pr_45_scale_corpus_assembles_with_exact_page_totals() {
    let started = std::time::Instant::now();
    let corpus = build_corpus(&pr_45_scale_state(), &LinkResolver::new(None));
    let elapsed = started.elapsed();

    assert_eq!(corpus.search.entries.len(), 976);
    assert_eq!(corpus.unfinished.item_count(), 42);

    // 576 rules, each naming exactly one requirement through the index.
    let produced_on_requirements: usize = corpus
        .requirements
        .iter()
        .map(|page| page.produced_rules.len())
        .sum();
    assert_eq!(produced_on_requirements, 576);
    let produced_on_decisions: usize = corpus
        .resolutions
        .iter()
        .map(|page| page.produced_rules.len())
        .sum();
    assert_eq!(produced_on_decisions, 576);

    // The rule pages read the same attribution inverted.
    let attributed_on_rules: usize = corpus
        .rules
        .iter()
        .map(|page| page.requirements.len())
        .sum();
    assert_eq!(attributed_on_rules, 576);

    // Requirements 165 and above name no requirement a rule refines; the
    // first 165 do, so each carries between three and four rules.
    assert!(corpus.requirements[..165]
        .iter()
        .all(|page| (3..=4).contains(&page.produced_rules.len())));
    assert!(corpus.requirements[165..]
        .iter()
        .all(|page| page.produced_rules.is_empty()));

    // The 216 refining requirements split evenly over the 12 anchors.
    let children_total: usize = corpus
        .requirements
        .iter()
        .map(|page| page.children.len())
        .sum();
    assert_eq!(children_total, 216);

    // The 186 finished requirements cite the 7 sources in rotation.
    let referenced_total: usize = corpus
        .sources
        .iter()
        .map(|page| page.referenced_requirements.len())
        .sum();
    assert_eq!(referenced_total, 186);

    // A coarse guard against a lookup that rescans the records: the whole
    // assembly runs in well under a second, so a regression to per-page
    // scans over the 976 records cannot hide here.
    assert!(
        elapsed < std::time::Duration::from_secs(10),
        "scale assembly took {elapsed:?}"
    );
}
