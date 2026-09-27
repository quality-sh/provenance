//! Tests for the Requirement candidate and decision cycle: the full
//! edit→submit→reject→revise→submit→approve round trip with exact versions
//! preserved, the inherited disposition gates, bypass refusals, feedback
//! atomicity, withdrawal, and frozen legacy history.

use crate::{layout::ProvenanceLayout, state_store::StateStore};
use camino::Utf8Path;
use provenance_core::{
    review::{CycleEntry, CycleFact},
    DispositionDecision, PromotionState, ScopeId, StableId,
};
use serde_json::{json, Value};

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}
fn req() -> StableId {
    StableId::new("req_a").unwrap()
}
fn open(root: &Utf8Path) -> StateStore {
    StateStore::new(ProvenanceLayout::new(root))
}
/// A manifest that allowlists "reviewer" plus one created record.
fn fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}],"disposition_actor_ids":["reviewer"]}"#,
    )
    .unwrap();
    open(Utf8Path::from_path(temp.path()).unwrap())
        .create_requirement(serde_json::from_value(json!({"scope_id":"default","id":"req_a","statement":"Statement v0","status":"discovery","depends_on":[],"supersedes":[]})).unwrap())
        .unwrap();
    temp
}
fn edit(store: &StateStore, request: &str, statement: &str) -> StableId {
    let etag = store.requirement_edit_state(&scope(), &req()).unwrap().etag;
    store
        .save_requirement(serde_json::from_value(json!({"request_id":request,"actor":"agent","expected_etag":etag,"update":{"scope_id":"default","id":"req_a","statement":statement},"relationships":null})).unwrap())
        .unwrap()
        .revision
}
fn submit(
    store: &StateStore,
    revises: Option<&StableId>,
    expected: Option<&str>,
) -> anyhow::Result<CycleEntry> {
    store.submit_requirement_review(
        serde_json::from_value(json!({
            "scope_id":"default","actor":"agent","requirement_id":"req_a",
            "title":"Title", "summary":"Summary", "source_ids":[],
            "evidence_references":[], "builds_on":[],
            "revises":revises,"expected_revision":expected
        }))
        .unwrap(),
    )
}
fn decide(
    store: &StateStore,
    proposal: &StableId,
    decision: &str,
    actor: &Value,
    extra: &Value,
) -> anyhow::Result<CycleEntry> {
    let mut value = json!({
        "scope_id":"default","actor":actor,"proposal_id":proposal,
        "decision":decision,"rationale":"Because"
    });
    for (key, extra_value) in extra.as_object().unwrap() {
        value[key] = extra_value.clone();
    }
    store.decide_requirement_review(serde_json::from_value(value).unwrap())
}
fn reviewer(id: &str) -> Value {
    json!({"identity_type":"human","id":id})
}
fn artifact() -> Value {
    json!({"canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_a"}})
}
fn feedback(body: &str) -> Value {
    json!({"feedback":{"role":"user","body":body}})
}
fn withdraw(store: &StateStore, proposal: &StableId) -> anyhow::Result<CycleEntry> {
    store.withdraw_requirement_review(serde_json::from_value(json!({"scope_id":"default","actor":"agent","proposal_id":proposal})).unwrap())
}
/// The fixture with one edit enrolled and one pending submission bound to it.
fn enrolled() -> (tempfile::TempDir, StateStore, StableId, StableId) {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    let revision = edit(&store, "edit-1", "Statement v1");
    let proposal = submit(&store, None, None).unwrap().proposal_id;
    (temp, store, revision, proposal)
}
fn state(store: &StateStore) -> provenance_core::review::RequirementDecisionState {
    store.requirement_decision_state(&scope(), &req()).unwrap()
}
fn binding_of(store: &StateStore, proposal: &StableId) -> (String, String) {
    let card = store
        .list_proposal_definitions(&scope())
        .unwrap()
        .into_iter()
        .find(|p| p.id == *proposal)
        .unwrap();
    let binding = card.record_revision.unwrap();
    (binding.revision.as_str().to_owned(), binding.content_digest)
}
/// Asserts a review operation is refused for the stated reason.
fn refused<T: std::fmt::Debug>(attempt: anyhow::Result<T>, needle: &str) {
    let message = attempt.unwrap_err().to_string();
    assert!(message.contains(needle), "{message}");
}

#[test]
fn full_cycle_persists_exact_versions_without_lifecycle_change() {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    let r1 = edit(&store, "edit-1", "Statement v1");
    let proposal_1 = submit(&store, None, Some(r1.as_str())).unwrap().proposal_id;
    let (prop1_revision, prop1_digest) = binding_of(&store, &proposal_1);
    assert_eq!(prop1_revision, r1.as_str());
    assert_eq!(state(&store).pending.unwrap().revision, r1);

    let rejection = decide(
        &store,
        &proposal_1,
        "rejected",
        &reviewer("reviewer"),
        &feedback("Tighten the statement"),
    )
    .unwrap();
    assert_eq!(rejection.fact, CycleFact::Decided);
    let after_rejection = state(&store);
    assert!(after_rejection.pending.is_none() && after_rejection.decisions.len() == 1);
    let recorded = &after_rejection.decisions[0];
    assert_eq!(recorded.disposition.decision, DispositionDecision::Rejected);
    assert_eq!(
        recorded.revision.as_ref().unwrap(),
        &r1,
        "the rejection records the exact reviewed version"
    );
    assert!(
        recorded.feedback_message_id.is_some(),
        "feedback published with its decision"
    );

    // Revise: a fresh Proposal answering one rejection, bound to the new
    // revision, then approve through the human existing-artifact path.
    let r2 = edit(&store, "edit-2", "Statement v2");
    let proposal_2 = submit(&store, Some(&proposal_1), Some(r2.as_str()))
        .unwrap()
        .proposal_id;
    let (prop2_revision, prop2_digest) = binding_of(&store, &proposal_2);
    assert_eq!(prop2_revision, r2.as_str());
    assert_ne!(
        prop1_digest, prop2_digest,
        "each submission binds its exact revision"
    );
    let prop2 = store
        .list_proposal_definitions(&scope())
        .unwrap()
        .into_iter()
        .find(|p| p.id == proposal_2)
        .unwrap();
    assert_eq!(prop2.revises.as_ref().unwrap(), &proposal_1);
    assert_eq!(
        prop2.revises_rejection.as_ref().unwrap(),
        rejection.disposition_id.as_ref().unwrap()
    );
    assert!(
        prop2.builds_on.is_empty(),
        "rejection links are not assertion lineage"
    );

    let approval = decide(
        &store,
        &proposal_2,
        "accepted",
        &reviewer("reviewer"),
        &artifact(),
    )
    .unwrap();
    let final_state = state(&store);
    assert_eq!(final_state.current_revision.as_ref().unwrap(), &r2);
    let acceptance = final_state.current_acceptance.as_ref().unwrap();
    assert_eq!(
        &acceptance.disposition.id,
        approval.disposition_id.as_ref().unwrap()
    );
    assert_eq!(
        acceptance.revision.as_ref().unwrap(),
        &r2,
        "approval accepts the reviewed version"
    );
    assert_eq!(
        final_state.decisions.len(),
        2,
        "history keeps every terminal decision"
    );
    assert_eq!(
        &final_state.decisions[0].disposition.id,
        rejection.disposition_id.as_ref().unwrap()
    );
    assert!(final_state.withdrawn.is_empty());
    assert_lifecycle_and_proposals_unchanged(&store, "Statement v2");
}

/// Approval never changes the record's lifecycle and never mutates a proposal.
fn assert_lifecycle_and_proposals_unchanged(store: &StateStore, statement: &str) {
    let record = &store.list_requirements(&scope()).unwrap()[0];
    assert_eq!(serde_json::to_value(&record.status).unwrap(), "discovery");
    assert_eq!(record.statement, statement);
    for proposal in store.list_proposal_definitions(&scope()).unwrap() {
        assert_eq!(proposal.promotion_state, PromotionState::Proposed);
    }
}

#[test]
fn server_creates_review_request_and_disposition_identities() {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    edit(&store, "edit-1", "Statement v1");
    let submission = submit(&store, None, None).unwrap();
    let proposal = submission.proposal_id.clone();
    assert_eq!(
        store
            .list_proposal_definitions(&scope())
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == proposal)
            .unwrap()
            .proposal_key,
        proposal.as_str()
    );
    let decision = decide(
        &store,
        &proposal,
        "accepted",
        &reviewer("reviewer"),
        &json!({
            "rationale": null,
            "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_a"}
        }),
    )
    .unwrap();
    assert_ne!(decision.request_id, submission.request_id);
    assert_ne!(decision.disposition_id.as_ref().unwrap(), &proposal);
}

#[test]
fn submission_gates_refuse_a_second_pending_or_unrevised_record() {
    let (_temp, store, _, _) = enrolled();
    refused(
        submit(&store, None, None),
        "already has a pending review submission",
    );
    assert_eq!(store.list_proposal_definitions(&scope()).unwrap().len(), 1);

    // Seed an unenrolled Requirement.
    let fresh = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(fresh.path()).unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}],"disposition_actor_ids":["reviewer"]}"#,
    )
    .unwrap();
    let store = open(Utf8Path::from_path(fresh.path()).unwrap());
    store
        .write_requirement(serde_json::from_value(json!({"scope_id":"default","id":"req_a","statement":"Statement v0","status":"discovery","depends_on":[],"supersedes":[]})).unwrap())
        .unwrap();
    refused(
        submit(&store, None, None),
        "requires a review revision",
    );
}

#[test]
fn stale_submission_and_stale_selection_are_refused() {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    let r1 = edit(&store, "edit-1", "Statement v1");
    edit(&store, "edit-2", "Statement v2");
    refused(
        submit(&store, None, Some(r1.as_str())),
        "stale submission",
    );
    assert!(store
        .list_proposal_definitions(&scope())
        .unwrap()
        .is_empty());

    let (_temp, store, _, proposal) = enrolled();
    edit(&store, "edit-2", "Statement v2");
    refused(
        decide(
            &store,
            &proposal,
            "accepted",
            &reviewer("reviewer"),
            &artifact(),
        ),
        "stale review selection",
    );
    assert!(store.list_dispositions(&scope()).unwrap().is_empty());
    refused(
        withdraw(&store, &proposal),
        "no longer current and pending",
    );
}

#[test]
fn unauthorized_actor_and_unqualified_acceptance_are_refused() {
    let (_temp, store, _, proposal) = enrolled();
    refused(
        decide(
            &store,
            &proposal,
            "accepted",
            &reviewer("intruder"),
            &artifact(),
        ),
        "allowlist",
    );
    assert!(store.list_dispositions(&scope()).unwrap().is_empty());

    refused(
        decide(
            &store,
            &proposal,
            "accepted",
            &json!({"identity_type":"agent","id":"agent-1"}),
            &json!({}),
        ),
        "asserted before disposition",
    );
    assert!(store.list_dispositions(&scope()).unwrap().is_empty());

    // The human existing-artifact path is the qualified exception.
    decide(
        &store,
        &proposal,
        "accepted",
        &reviewer("reviewer"),
        &artifact(),
    )
    .unwrap();
    assert_eq!(store.list_dispositions(&scope()).unwrap().len(), 1);
}

#[test]
fn one_terminal_disposition_per_proposal() {
    let (_temp, store, _, proposal) = enrolled();
    // Rejection keeps a nonempty rationale and permits absent feedback.
    decide(
        &store,
        &proposal,
        "rejected",
        &reviewer("reviewer"),
        &json!({}),
    )
    .unwrap();
    refused(
        decide(
            &store,
            &proposal,
            "accepted",
            &reviewer("reviewer"),
            &artifact(),
        ),
        "no longer pending",
    );
    assert_eq!(store.list_dispositions(&scope()).unwrap().len(), 1);
}

#[test]
fn feedback_publishes_with_the_decision_or_neither() {
    let (_temp, store, _, proposal) = enrolled();
    let falsified = json!({"feedback":{"role":"user","body":"Comments"},"declared_by":"mallory"});
    refused(
        decide(
            &store,
            &proposal,
            "rejected",
            &reviewer("reviewer"),
            &falsified,
        ),
        "declared_by",
    );
    assert!(
        store.list_dispositions(&scope()).unwrap().is_empty(),
        "no decision without its feedback"
    );
    assert!(
        store.list_threads(&scope()).unwrap().is_empty(),
        "no feedback without its decision"
    );
    assert!(store.list_messages(&scope()).unwrap().is_empty());

    decide(
        &store,
        &proposal,
        "rejected",
        &reviewer("reviewer"),
        &feedback("Comments"),
    )
    .unwrap();
    assert_eq!(store.list_dispositions(&scope()).unwrap().len(), 1);
    assert_eq!(store.list_threads(&scope()).unwrap().len(), 1);
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 1);
    assert!(state(&store).decisions[0].feedback_message_id.is_some());
}

#[test]
fn withdrawal_preserves_the_candidate_and_allows_a_fresh_submission() {
    let (_temp, store, _, proposal_1) = enrolled();
    withdraw(&store, &proposal_1).unwrap();
    let withdrawn = state(&store);
    assert_eq!(withdrawn.withdrawn, vec![proposal_1.clone()]);
    assert!(withdrawn.pending.is_none() && withdrawn.decisions.is_empty());
    assert!(
        store
            .list_proposal_definitions(&scope())
            .unwrap()
            .iter()
            .any(|p| p.id == proposal_1),
        "withdrawal keeps the candidate, its feedback, and the graph record"
    );

    // Withdrawal is not rejection: a fresh candidate needs no predecessor.
    let proposal_2 = submit(&store, None, None).unwrap().proposal_id;
    decide(
        &store,
        &proposal_2,
        "rejected",
        &reviewer("reviewer"),
        &json!({}),
    )
    .unwrap();
    refused(
        withdraw(&store, &proposal_2),
        "no longer current and pending",
    );
}

mod bypass_tests;
#[path = "decision_tests/conflict_tests.rs"]
mod conflict_tests;

#[test]
fn legacy_unbound_decisions_read_correctly_and_stay_frozen() {
    let (_temp, store, _, _) = enrolled();
    store.create_proposal_card(serde_json::from_value(json!({
        "scope_id":"default","id":"prop-legacy","proposal_key":"legacy","proposal_type":"requirement_candidate",
        "title":"Legacy","summary":"Summary","traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_a"},
        "source_ids":[],"evidence_references":[],"supporting_claim_ids":[]},"builds_on":[],
        "promotion_state":"proposed"})).unwrap()).unwrap();
    store
        .create_disposition(serde_json::from_value(json!({
            "scope_id":"default","id":"disp-legacy","proposal_id":"prop-legacy","decision":"accepted",
            "rationale":"Ratified offline","actor":{"identity_type":"human","id":"reviewer"},
            "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_a"}})).unwrap())
        .unwrap();

    let recorded = state(&store);
    assert_eq!(recorded.decisions.len(), 1);
    assert!(
        recorded.decisions[0].revision.is_none(),
        "a legacy disposition binds no revision"
    );
    assert!(
        recorded.current_acceptance.is_none(),
        "a legacy acceptance attests no current content"
    );

    edit(&store, "edit-2", "Statement v2");
    assert!(
        state(&store).current_acceptance.is_none(),
        "editing keeps historical acceptance only"
    );

    refused(
        store.create_disposition(serde_json::from_value(json!({
            "scope_id":"default","id":"disp-legacy-2","proposal_id":"prop-legacy","decision":"rejected",
            "rationale":"Rewriting history","actor":{"identity_type":"human","id":"reviewer"}})).unwrap()),
        "authoritative disposition",
    );
    assert_eq!(store.list_dispositions(&scope()).unwrap().len(), 1);
}
