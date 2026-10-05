use crate::{layout::ProvenanceLayout, state_store::StateStore};
use camino::Utf8Path;
use provenance_core::{
    review::{CycleEntry, CycleFact},
    DispositionDecision, NodeType, PromotionState, ScopeId, StableId,
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
/// A repository that allowlists "reviewer" plus one enrolled record.
fn fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    crate::test_support::allow_reviewer(&layout);
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    store
        .write_requirement(serde_json::from_value(json!({"scope_id":"default","id":"req_a","statement":"Statement v0","status":"discovery","depends_on":[],"supersedes":[]})).unwrap())
        .unwrap();
    let etag = store.requirement_edit_state(&scope(), &req()).unwrap().etag;
    store
        .save_requirement(serde_json::from_value(json!({"request_id":"fixture-enroll","actor":"agent","expected_etag":etag,"update":{"scope_id":"default","id":"req_a"},"relationships":null})).unwrap())
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
    store.submit_record_review(
        serde_json::from_value(json!({
            "scope_id":"default","actor":"agent","record_kind":"requirement","record_id":"req_a",
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
    let rationale = (decision != "accepted").then_some("Because");
    let mut value = json!({
        "scope_id":"default","actor":actor,"proposal_id":proposal,
        "decision":decision,"rationale":rationale
    });
    for (key, extra_value) in extra.as_object().unwrap() {
        value[key] = extra_value.clone();
    }
    store.decide_record_review(serde_json::from_value(value).unwrap())
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
    store.withdraw_record_review(
        serde_json::from_value(
            json!({"scope_id":"default","actor":"agent","proposal_id":proposal}),
        )
        .unwrap(),
    )
}
/// The fixture with one edit enrolled and one pending submission bound to it.
fn enrolled() -> (tempfile::TempDir, StateStore, StableId, StableId) {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    let revision = edit(&store, "edit-1", "Statement v1");
    let proposal = automatic_submission(&store).proposal_id;
    (temp, store, revision, proposal)
}
fn state(store: &StateStore) -> provenance_core::review::RequirementDecisionState {
    store.requirement_decision_state(&scope(), &req()).unwrap()
}
fn automatic_submission(store: &StateStore) -> CycleEntry {
    let proposal = state(store).pending.unwrap().proposal_id;
    store
        .cycle_entries(&scope())
        .unwrap()
        .into_iter()
        .find(|entry| entry.proposal_id == proposal && entry.fact == CycleFact::Submitted)
        .unwrap()
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
/// This flow follows one record through rejection, revision, and approval.
#[provenance_macros::verifies("rule_approval_accepts_reviewed_version", examples)]
#[provenance_macros::verifies("rule_revised_item_requires_new_review", examples)]
fn full_cycle_persists_exact_versions_without_lifecycle_change() {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    let r1 = edit(&store, "edit-1", "Statement v1");
    let proposal_1 = automatic_submission(&store).proposal_id;
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

    // Revise: a fresh Proposal is bound to the new revision. Then approve it
    // through the human existing-artifact path.
    let r2 = edit(&store, "edit-2", "Revised statement");
    let proposal_2 = automatic_submission(&store).proposal_id;
    let (prop2_revision, prop2_digest) = binding_of(&store, &proposal_2);
    assert_eq!(prop2_revision, r2.as_str());
    assert_ne!(
        prop1_digest, prop2_digest,
        "each submission binds its exact revision"
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
    assert_eq!(final_state.withdrawn, [] as [provenance_core::StableId; 0]);
    assert_lifecycle_and_proposals_unchanged(&store, "Revised statement");
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
/// Implementation aid: pins submission guards before the review-cycle state is refactored.
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
    crate::test_support::allow_reviewer(&layout);
    let store = open(Utf8Path::from_path(fresh.path()).unwrap());
    store
        .write_requirement(serde_json::from_value(json!({"scope_id":"default","id":"req_a","statement":"Statement v0","status":"discovery","depends_on":[],"supersedes":[]})).unwrap())
        .unwrap();
    refused(submit(&store, None, None), "requires a review revision");
}

#[test]
#[provenance_macros::verifies("rule_review_conflict_not_merged", examples)]
fn stale_submission_and_stale_selection_are_refused() {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    let r1 = edit(&store, "edit-1", "Statement v1");
    edit(&store, "edit-2", "Revised statement");
    refused(submit(&store, None, Some(r1.as_str())), "stale submission");
    assert_eq!(store.list_proposal_definitions(&scope()).unwrap().len(), 2);

    let (_temp, store, _, proposal) = enrolled();
    edit(&store, "edit-2", "Revised statement");
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
    assert_eq!(
        store.list_dispositions(&scope()).unwrap(),
        [] as [provenance_core::DispositionRecord; 0]
    );
    refused(withdraw(&store, &proposal), "no longer current and pending");
}

#[test]
#[provenance_macros::verifies("rule_disposition_actor_allowlist", examples)]
fn an_unauthorized_actor_cannot_accept_a_review() {
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
    assert_eq!(
        store.list_dispositions(&scope()).unwrap(),
        [] as [provenance_core::DispositionRecord; 0]
    );
}

#[test]
#[provenance_macros::verifies("rule_disposition_write_gate", examples)]
fn an_unqualified_agent_cannot_accept_a_review() {
    let (_temp, store, _, proposal) = enrolled();
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
    assert_eq!(
        store.list_dispositions(&scope()).unwrap(),
        [] as [provenance_core::DispositionRecord; 0]
    );
}

#[test]
#[provenance_macros::verifies("rule_review_approval_has_no_rationale", examples)]
fn an_approval_takes_no_rationale() {
    let (_temp, store, _, proposal) = enrolled();
    refused(
        decide(
            &store,
            &proposal,
            "accepted",
            &reviewer("reviewer"),
            &json!({
                "rationale":"No rationale is valid for an approval.",
                "canonical_artifact":{
                    "artifact_type":"requirement", "artifact_id":"req_a"
                }
            }),
        ),
        "approval does not take a rationale",
    );
    assert_eq!(
        store.list_dispositions(&scope()).unwrap(),
        [] as [provenance_core::DispositionRecord; 0]
    );

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
#[provenance_macros::verifies("rule_disposition_write_gate", examples)]
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
/// Implementation aid: pins atomic feedback publication before journal consolidation.
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
    assert_eq!(
        store.list_messages(&scope()).unwrap(),
        [] as [provenance_core::Message; 0]
    );

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
#[provenance_macros::verifies("rule_rejection_keeps_graph_record", examples)]
fn withdrawal_does_not_remove_or_retire_the_graph_record() {
    let (_temp, store, _, proposal_1) = enrolled();
    let before = store.list_requirements(&scope()).unwrap().remove(0);

    withdraw(&store, &proposal_1).unwrap();
    let requirement = store.list_requirements(&scope()).unwrap().remove(0);
    assert_eq!(
        requirement.id,
        req(),
        "withdrawal must keep Requirement req_a"
    );
    assert_eq!(
        requirement.status, before.status,
        "withdrawal must not retire the Requirement graph record"
    );
}

#[test]
/// This flow withdraws one submission, rejects the next, and refuses another withdrawal.
fn withdrawal_allows_a_fresh_review_cycle() {
    let (_temp, store, _, proposal_1) = enrolled();
    withdraw(&store, &proposal_1).unwrap();
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

#[test]
/// Implementation aid: keeps JavaScript-safe receipt ordering within its numeric budget.
fn review_finding_withdrawn_and_resubmitted_receipts_have_safe_sequences() {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    edit(&store, "edit-1", "Statement v1");
    let proposal = state(&store).pending.unwrap().proposal_id;

    let withdrawn = withdraw(&store, &proposal).unwrap();
    let resubmitted = submit(&store, None, None).unwrap();

    assert!(withdrawn.sequence < resubmitted.sequence);
    assert!(resubmitted.sequence < (1_u64 << 53));
}

#[test]
/// Implementation aid: rejects forged journal addresses before decision-state reads.
fn cycle_receipt_refuses_a_different_record_kind_than_its_proposal() {
    let (_temp, store, _, proposal) = enrolled();
    let submitted = automatic_submission(&store);
    let forged = CycleEntry {
        id: super::journal::new_id(),
        record_kind: NodeType::Source,
        sequence: submitted.sequence + 1,
        fact: CycleFact::Withdrawn,
        request_id: super::journal::new_id(),
        intent_digest: "sha256:forged-kind".into(),
        ..submitted
    };
    super::decision_state::write_receipt(&store, &forged).unwrap();

    refused(
        store.validated_cycle_entries(&scope()),
        "cycle entry address does not match its proposal target",
    );
    refused(
        store.requirement_decision_state(&scope(), &req()),
        "cycle entry address does not match its proposal target",
    );
    refused(
        withdraw(&store, &proposal),
        "cycle entry address does not match its proposal target",
    );
}

mod bypass_tests;
mod canonical_artifacts;
#[path = "decision_tests/conflict_tests.rs"]
mod conflict_tests;
#[path = "decision_tests/legacy.rs"]
mod legacy;
#[path = "decision_tests/rejection_tests.rs"]
mod rejection_tests;
