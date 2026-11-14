use super::*;
use provenance_core::{
    review::{RequestedStatus, ReviewRecord},
    NodeType, Rule, RuleStatus,
};

const ARCHIVE_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn rule_id() -> StableId {
    StableId::new("rule_a").unwrap()
}

fn read_rule(store: &StateStore) -> Rule {
    match crate::families::review_families::record(store, &scope(), NodeType::Rule, &rule_id())
        .unwrap()
    {
        ReviewRecord::Rule(rule) => rule,
        other => panic!("expected a Rule, got {other:?}"),
    }
}

fn rule_state(store: &StateStore) -> provenance_core::review::RequirementDecisionState {
    store
        .record_decision_state(&scope(), NodeType::Rule, &rule_id())
        .unwrap()
}

fn create(store: &StateStore, status: &str) -> anyhow::Result<Rule> {
    store.request_rule_creation(
        serde_json::from_value(json!({
            "scope_id":"default","id":"rule_a","statement":"The rule holds.",
            "requirement_ids":["req_a"],"resolution_ids":[],"status":status,"severity":"high"
        }))
        .unwrap(),
    )
}

fn update(store: &StateStore, fields: &Value) -> anyhow::Result<Rule> {
    let mut value = json!({"scope_id":"default","id":"rule_a"});
    for (key, field) in fields.as_object().unwrap() {
        value[key] = field.clone();
    }
    store.request_rule_update(serde_json::from_value(value).unwrap())
}

fn accept(store: &StateStore, proposal: &StableId) {
    let artifact = json!({"canonical_artifact":{"artifact_type":"rule","artifact_id":"rule_a"}});
    decide(
        store,
        proposal,
        "accepted",
        &reviewer("reviewer"),
        &artifact,
    )
    .unwrap();
}

/// A draft Rule whose creation submission a reviewer accepted.
fn accepted_draft_rule() -> (tempfile::TempDir, StateStore) {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    create(&store, "draft").unwrap();
    accept(&store, &rule_state(&store).pending.unwrap().proposal_id);
    (temp, store)
}

fn card(store: &StateStore, proposal: &StableId) -> provenance_core::ProposalCard {
    store
        .list_proposal_definitions(&scope())
        .unwrap()
        .into_iter()
        .find(|card| card.id == *proposal)
        .unwrap()
}

/// The pending submission and the status that its binding requests.
fn pending_request(store: &StateStore) -> (StableId, provenance_core::review::StatusRequest) {
    let pending = rule_state(store).pending.expect("a pending submission");
    let request = card(store, &pending.proposal_id)
        .record_revision
        .and_then(|binding| binding.status_request)
        .expect("a status request");
    (pending.proposal_id, request)
}

fn title_of(store: &StateStore, proposal: &StableId) -> String {
    card(store, proposal).title
}

#[test]
#[provenance_macros::verifies("rule_status_change_waits_for_review", examples)]
fn activation_opens_a_status_request_and_keeps_the_status() {
    let (_temp, store) = accepted_draft_rule();

    let returned = update(&store, &json!({"status":"active"})).unwrap();

    assert_eq!(returned.status, RuleStatus::Draft);
    assert_eq!(read_rule(&store).status, RuleStatus::Draft);
    let (proposal, request) = pending_request(&store);
    assert_eq!(request.status, RequestedStatus::Active);
    assert_eq!(title_of(&store, &proposal), "Set status to active");
    assert_eq!(
        rule_state(&store).pending.unwrap().status_request,
        Some(request)
    );
}

#[test]
#[provenance_macros::verifies("rule_status_approval_sets_status", examples)]
fn approval_sets_the_requested_status() {
    let (_temp, store) = accepted_draft_rule();
    update(&store, &json!({"status":"active"})).unwrap();
    let (proposal, _) = pending_request(&store);

    accept(&store, &proposal);

    assert_eq!(read_rule(&store).status, RuleStatus::Active);
    let state = rule_state(&store);
    assert!(state.pending.is_none());
    assert!(state.current_acceptance.is_some());
    let last = state.decisions.last().unwrap();
    assert_eq!(last.disposition.proposal_id, proposal);
    assert_eq!(last.disposition.decision, DispositionDecision::Accepted);
}

#[test]
#[provenance_macros::verifies("rule_status_approval_sets_status", examples)]
fn approval_of_an_archive_request_sets_the_archive_commit() {
    let (_temp, store) = accepted_draft_rule();
    update(
        &store,
        &json!({"status":"archived","archived_in_commit":{"commit":ARCHIVE_COMMIT}}),
    )
    .unwrap();
    let (proposal, request) = pending_request(&store);
    assert_eq!(request.status, RequestedStatus::Archived);
    assert_eq!(read_rule(&store).archived_in_commit, None);

    accept(&store, &proposal);

    let rule = read_rule(&store);
    assert_eq!(rule.status, RuleStatus::Archived);
    assert_eq!(rule.archived_in_commit.unwrap().commit, ARCHIVE_COMMIT);
}

#[test]
#[provenance_macros::verifies("rule_status_refusal_keeps_status", examples)]
fn rejection_and_withdrawal_keep_the_status() {
    let (_temp, store) = accepted_draft_rule();
    update(&store, &json!({"status":"active"})).unwrap();
    let (rejected, _) = pending_request(&store);
    decide(
        &store,
        &rejected,
        "rejected",
        &reviewer("reviewer"),
        &json!({}),
    )
    .unwrap();
    assert_eq!(read_rule(&store).status, RuleStatus::Draft);

    update(&store, &json!({"status":"active"})).unwrap();
    let (withdrawn, _) = pending_request(&store);
    withdraw(&store, &withdrawn).unwrap();

    assert_eq!(read_rule(&store).status, RuleStatus::Draft);
    assert!(rule_state(&store).pending.is_none());
}

#[test]
#[provenance_macros::verifies("rule_status_request_changes_one_field", examples)]
fn a_status_request_with_another_field_is_refused() {
    let (_temp, store) = accepted_draft_rule();
    let before = read_rule(&store);

    refused(
        update(&store, &json!({"status":"active","name":"New name"})),
        "cannot change other fields",
    );

    assert_eq!(read_rule(&store), before);
    assert!(rule_state(&store).pending.is_none());
}

#[test]
fn a_pending_submission_refuses_a_status_request() {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    create(&store, "draft").unwrap();

    refused(
        update(&store, &json!({"status":"active"})),
        "already has a pending review submission",
    );
    assert_eq!(read_rule(&store).status, RuleStatus::Draft);
}

#[test]
fn other_statuses_stay_direct_writes() {
    let (_temp, store) = accepted_draft_rule();

    update(&store, &json!({"status":"deprecated"})).unwrap();

    assert_eq!(read_rule(&store).status, RuleStatus::Deprecated);
    assert!(rule_state(&store).pending.is_none());
}

#[test]
#[provenance_macros::verifies("rule_new_active_rule_waits_for_review", examples)]
fn a_new_active_rule_has_the_review_status_until_approval() {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());

    let created = create(&store, "active").unwrap();

    assert_eq!(created.status, RuleStatus::Review);
    assert_eq!(read_rule(&store).status, RuleStatus::Review);
    let (proposal, request) = pending_request(&store);
    assert_eq!(request.status, RequestedStatus::Active);

    accept(&store, &proposal);

    assert_eq!(read_rule(&store).status, RuleStatus::Active);
}

#[test]
fn a_new_archived_rule_is_refused() {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());

    refused(
        create(&store, "archived"),
        "cannot have the archived status",
    );
}

/// A fixture with the proposed Resolution `res_a` and a creation of
/// `rule_a` with `status` that cites it.
fn create_citing_proposed_resolution(
    status: &str,
) -> (tempfile::TempDir, StateStore, anyhow::Result<Rule>) {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    store
        .create_resolution(
            serde_json::from_value(json!({
                "scope_id":"default","id":"res_a","title":"Pick A",
                "requirement_ids":["req_a"],"supersedes":[],"position":"Pick A.",
                "rationale":"A is simpler.","status":"proposed","context":null,"inputs":[],
                "made_by":null,"approved_by":null,"approved_at":null,
                "origin_thread":null,"origin_message":null
            }))
            .unwrap(),
        )
        .unwrap();
    let created = store.request_rule_creation(
        serde_json::from_value(json!({
            "scope_id":"default","id":"rule_a","statement":"The rule holds.",
            "requirement_ids":["req_a"],"resolution_ids":["res_a"],"status":status,
            "severity":"high"
        }))
        .unwrap(),
    );
    (temp, store, created)
}

#[test]
#[provenance_macros::verifies("rule_status_request_refused_like_direct_write", examples)]
fn a_new_active_rule_citing_a_proposed_resolution_is_refused() {
    let (_temp, store, created) = create_citing_proposed_resolution("active");

    refused(created, "cites resolution res_a, which is proposed");

    assert_eq!(store.list_rules(&scope()).unwrap(), [] as [Rule; 0]);
}

#[test]
#[provenance_macros::verifies("rule_status_request_refused_like_direct_write", examples)]
fn the_direct_write_refusal_comes_before_the_pending_check() {
    let (_temp, store, created) = create_citing_proposed_resolution("draft");
    created.unwrap();
    let pending = rule_state(&store).pending.expect("the creation submission");

    refused(
        update(&store, &json!({"status":"active"})),
        "cites resolution res_a, which is proposed",
    );

    assert_eq!(read_rule(&store).status, RuleStatus::Draft);
    let after = rule_state(&store).pending.expect("the creation submission");
    assert_eq!(after.proposal_id, pending.proposal_id);
    assert!(after.status_request.is_none());
}

#[test]
#[provenance_macros::verifies("rule_status_request_refused_like_direct_write", examples)]
fn an_activation_citing_a_proposed_resolution_opens_no_submission() {
    let (_temp, store, created) = create_citing_proposed_resolution("draft");
    created.unwrap();
    accept(&store, &rule_state(&store).pending.unwrap().proposal_id);

    refused(
        update(&store, &json!({"status":"active"})),
        "cites resolution res_a, which is proposed",
    );

    assert_eq!(read_rule(&store).status, RuleStatus::Draft);
    assert!(rule_state(&store).pending.is_none());
}
