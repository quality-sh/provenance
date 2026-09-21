//! `evidence` and `resolve_symbol` pages hydrate only the records they
//! serve.
//!
//! Both reads answer from tables whose match set can be arbitrarily
//! wide: the page must be chosen by id, the binding candidates must give
//! up their rule ids without a decode, and a cleared review must be cut
//! in the candidate query so it neither serves nor consumes a page slot.
//! Every count here reads the `query_row_hydrated` probe, so a bound is
//! proved, not inferred from a final answer.

use super::hydration_support::{
    append_binding, append_review, counted, evidence_query, resolve_query, OVERSIZED_BYTES,
};
use super::{root_of, seeded_store};
use crate::cache::tests::fixtures::create_rule_of;
use crate::operations::queries;
use crate::operations::read_policy::ReadPolicy;
use provenance_core::protocol::{EvidenceResult, ResolveSymbolResult};
use provenance_macros::verifies;

fn binding_ids(result: &EvidenceResult) -> Vec<String> {
    result
        .implementation_bindings
        .iter()
        .map(|binding| binding.id.as_str().to_string())
        .collect()
}

fn rule_ids(result: &ResolveSymbolResult) -> Vec<String> {
    result
        .rules
        .iter()
        .map(|node| node.id().as_str().to_string())
        .collect()
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn evidence_hydrates_only_the_served_page() {
    let (dir, store, scope) = seeded_store();
    create_rule_of(&store, &scope, "rule_overtime", "req_overtime");
    for index in 0..30 {
        append_binding(
            &store,
            &scope,
            &format!("bind_{index:03}"),
            "rule_overtime",
            "pay",
            0,
        );
    }
    let (answer, hydrations) = counted(queries::evidence(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        evidence_query(3),
    ))
    .await;
    let result = answer.unwrap().result;
    assert_eq!(binding_ids(&result), ["bind_000", "bind_001", "bind_002"]);
    assert!(result.implementation_bindings_has_more);
    assert!(result.has_more);
    assert!(hydrations <= 4, "three page rows for thirty matches");
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn an_oversized_binding_past_the_page_cannot_refuse_evidence() {
    let (dir, store, scope) = seeded_store();
    create_rule_of(&store, &scope, "rule_overtime", "req_overtime");
    append_binding(&store, &scope, "bind_000", "rule_overtime", "pay", 0);
    append_binding(&store, &scope, "bind_001", "rule_overtime", "pay", 0);
    append_binding(
        &store,
        &scope,
        "bind_999",
        "rule_overtime",
        "pay",
        OVERSIZED_BYTES,
    );
    let (answer, hydrations) = counted(queries::evidence(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        evidence_query(2),
    ))
    .await;
    let result = answer
        .expect("the page answers past an oversized record")
        .result;
    assert_eq!(binding_ids(&result), ["bind_000", "bind_001"]);
    assert!(result.implementation_bindings_has_more);
    assert!(hydrations <= 3, "the oversized row must never decode");
}

/// The open-only cut belongs to the candidate query: cleared reviews
/// between open ones neither serve nor consume a page slot.
#[tokio::test]
async fn evidence_pages_only_the_open_reviews() {
    let (dir, store, scope) = seeded_store();
    create_rule_of(&store, &scope, "rule_overtime", "req_overtime");
    for (id, cleared_at) in [
        ("review_a", None),
        ("review_b", Some(5)),
        ("review_c", None),
        ("review_d", Some(5)),
        ("review_e", None),
    ] {
        append_review(&store, &scope, id, cleared_at);
    }
    let answer = queries::evidence(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        evidence_query(2),
    )
    .await
    .unwrap()
    .result;
    let reviews: Vec<String> = answer
        .reviews
        .iter()
        .map(|review| review.id.as_str().to_string())
        .collect();
    assert_eq!(reviews, ["review_a", "review_c"]);
    assert!(answer.reviews_has_more);
    assert!(answer.review_required);
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn resolve_symbol_hydrates_only_the_rule_page() {
    let store = super::comparison::test_stores::seeded_queries();
    let state = store.state_store();
    create_rule_of(&state, &store.scope, "rule_overtime", "req_overtime");
    create_rule_of(&state, &store.scope, "rule_audit", "req_overtime");
    create_rule_of(&state, &store.scope, "rule_site_000", "req_overtime");
    create_rule_of(&state, &store.scope, "rule_site_001", "req_overtime");
    for index in 0..30 {
        append_binding(
            &state,
            &store.scope,
            &format!("bind_{index:03}"),
            &format!("rule_site_{index:03}"),
            "pay",
            0,
        );
    }
    append_binding(&state, &store.scope, "bind_audit", "rule_audit", "audit", 0);

    let (answer, hydrations) = counted(queries::resolve_symbol(
        Some(store.root.clone()),
        &store.scope,
        ReadPolicy::default(),
        resolve_query(None, 2),
    ))
    .await;
    let result = answer.unwrap().result;
    assert_eq!(
        rule_ids(&result),
        ["rule_audit", "rule_overtime"],
        "candidates sort by id"
    );
    assert!(result.has_more, "two served rule records remain past the page");
    assert!(hydrations <= 3, "binding rows must never decode");

    let (answer, hydrations) = counted(queries::resolve_symbol(
        Some(store.root.clone()),
        &store.scope,
        ReadPolicy::default(),
        resolve_query(Some("pay"), 2),
    ))
    .await;
    let result = answer.unwrap().result;
    assert_eq!(
        rule_ids(&result),
        ["rule_overtime", "rule_site_000"],
        "the symbol filters"
    );
    assert!(result.has_more, "rule_site_001 remains past the page");
    assert!(hydrations <= 3, "binding rows must never decode");
}
