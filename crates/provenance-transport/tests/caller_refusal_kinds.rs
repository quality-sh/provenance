//! A refusal that the input of a caller causes has a declared kind and a
//! status that is not 500, for writes that go through the review store.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
    pub mod resource_http;
}

use provenance_macros::verifies;
use serde_json::{json, Value};
use support::records::{allow_reviewer, Repository};
use support::resource_http::{call, host};

/// The status and the error object of one answer, under the name of its row.
fn row(name: &str, (status, body): (u16, Value)) -> Value {
    json!({"row": name, "status": status, "error": body["error"]})
}

fn contribution() -> Value {
    json!({
        "id":"contribution_shared",
        "target":{"artifact_type":"requirement","artifact_id":"req_shared"},
        "participant_slot":"reviewer", "stance":"support",
        "strongest_finding":"Original finding", "evidence_references":[],
        "material_claims":[], "risks":[], "objections":[], "challenges":[],
        "suggested_artifact_changes":[], "unsupported_recommendations":[],
        "uncertainty":{"level":"low","rationale":"Direct evidence"},
        "open_questions":[]
    })
}

fn synthesis() -> Value {
    json!({
        "id":"synthesis_shared",
        "target":{"artifact_type":"requirement","artifact_id":"req_shared"},
        "summary":"Original summary", "consensus":[], "contested_claims":[],
        "minority_objections":[], "evidence_gaps":[], "unsupported_speculation":[],
        "open_questions":[], "suggested_artifacts":[], "required_human_decisions":[]
    })
}

#[tokio::test]
#[verifies("rule_caller_refusal_has_declared_kind", examples)]
async fn a_second_create_of_an_ideation_record_already_exists() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let mut rows = Vec::new();
    for (path, data) in [
        ("/contributions", contribution()),
        ("/synthesis-packets", synthesis()),
    ] {
        let body = json!({ "data": data });
        let (status, created) = call(&host, "POST", path, Some(body.clone())).await;
        assert_eq!(status, 200, "{created}");
        rows.push(row(path, call(&host, "POST", path, Some(body)).await));
    }
    assert_eq!(
        Value::Array(rows),
        json!([
            {"error":{"kind":"already_exists"},"row":"/contributions","status":409},
            {"error":{"kind":"already_exists"},"row":"/synthesis-packets","status":409}
        ])
    );
}

#[tokio::test]
#[verifies("rule_caller_refusal_has_declared_kind", examples)]
async fn an_origin_that_does_not_exist_is_a_missing_reference() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let mut rows = Vec::new();
    for (id, field, value) in [
        ("req_from_thread", "origin_thread", "thread_missing"),
        ("req_from_message", "origin_message", "msg_missing"),
    ] {
        let mut data = json!({
            "actor":"reviewer", "id":id, "statement":"The system stores each record.",
            "status":"discovery", "depends_on":[], "supersedes":[]
        });
        data[field] = json!(value);
        rows.push(row(
            field,
            call(
                &host,
                "POST",
                "/requirements",
                Some(json!({ "data": data })),
            )
            .await,
        ));
    }
    assert_eq!(
        Value::Array(rows),
        json!([
            {"error":{"kind":"missing_reference"},"row":"origin_thread","status":400},
            {"error":{"kind":"missing_reference"},"row":"origin_message","status":400}
        ])
    );
}

#[tokio::test]
#[verifies("rule_caller_refusal_has_declared_kind", examples)]
async fn a_topic_or_question_discussion_that_names_an_owner_is_an_invalid_update() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let mut rows = Vec::new();
    for parent in ["/topics/topic_shared", "/questions/question_shared"] {
        let body = json!({"data":{
            "actor":"reviewer", "declared_by":"agent", "role":"user", "body":"A concern."
        }});
        rows.push(row(
            parent,
            call(&host, "POST", &format!("{parent}/discussions"), Some(body)).await,
        ));
    }
    let refusal = json!({"kind":"invalid_update",
        "message":"a topic or question parent takes no declared owner"});
    assert_eq!(
        Value::Array(rows),
        json!([
            {"error":refusal,"row":"/topics/topic_shared","status":400},
            {"error":refusal,"row":"/questions/question_shared","status":400}
        ])
    );
}

/// Evidence for an assertion of `proposal_decided`: a contribution with one
/// claim and a synthesis packet that suggests the proposal.
fn evidence() -> [(&'static str, Value); 2] {
    let target = json!({"artifact_type":"requirement","artifact_id":"req_shared"});
    [
        (
            "/contributions",
            json!({"data":{
                "id":"contribution_evidence", "target":target,
                "participant_slot":"reviewer", "stance":"support", "strongest_finding":"Observed",
                "evidence_references":[{"reference_id":"evidence_a","evidence_type":"source","summary":"Pinned"}],
                "material_claims":[{"claim_id":"claim_a","statement":"Observed",
                    "evidence_type":"source","evidence_reference_ids":["evidence_a"]}],
                "risks":[], "objections":[], "challenges":[], "suggested_artifact_changes":[],
                "unsupported_recommendations":[], "uncertainty":{"level":"low","rationale":"Direct"},
                "open_questions":[]
            }}),
        ),
        (
            "/synthesis-packets",
            json!({"data":{
                "id":"synthesis_evidence", "target":target, "summary":"Adjudicated",
                "consensus":[], "contested_claims":[], "minority_objections":[],
                "evidence_gaps":[], "unsupported_speculation":[], "open_questions":[],
                "suggested_artifacts":[{"proposal_id":"proposal_decided","proposal_key":"decided",
                    "proposal_type":"requirement_candidate","summary":"Candidate",
                    "origin_participant_slots":["reviewer"]}],
                "required_human_decisions":[]
            }}),
        ),
    ]
}

fn assertion(id: &str) -> Value {
    json!({"data":{"id":id, "synthesis_packet_id":"synthesis_evidence",
        "supporting_claim_ids":["claim_a"]}})
}

fn disposition(id: &str) -> Value {
    json!({"data":{"id":id, "decision":"rejected", "rationale":"Reviewed",
        "actor":{"identity_type":"human","id":"reviewer"}}})
}

/// A second assertion or disposition, and an assertion or a disposition of a
/// proposal that has a disposition, have a declared kind. No route creates a
/// proposal that a packet qualifies, so the test creates the proposal and its
/// assertion in one store call, as a swarm landing does.
#[tokio::test]
#[verifies("rule_caller_refusal_has_declared_kind", examples)]
async fn a_second_assertion_or_disposition_has_a_declared_kind() {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let host = host(&repo, true);
    for (path, body) in evidence() {
        let (status, created) = call(&host, "POST", path, Some(body)).await;
        assert_eq!(status, 200, "{path}: {created}");
    }
    let proposal = serde_json::from_value(json!({
        "scope_id":"default", "id":"proposal_decided", "proposal_key":"decided",
        "proposal_type":"requirement_candidate", "title":"Decided",
        "summary":"Clarify the shared requirement.",
        "traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_shared"},
            "source_ids":[], "evidence_references":[], "supporting_claim_ids":["claim_a"]},
        "builds_on":[], "promotion_state":"proposed"
    }))
    .unwrap();
    let mut first = assertion("assertion_a")["data"].clone();
    first["scope_id"] = json!("default");
    first["proposal_id"] = json!("proposal_decided");
    repo.open()
        .store()
        .create_asserted_proposal(proposal, serde_json::from_value(first).unwrap())
        .unwrap();
    let assertions = "/proposals/proposal_decided/assertions";
    let dispositions = "/proposals/proposal_decided/dispositions";
    let second = call(&host, "POST", assertions, Some(assertion("assertion_a"))).await;
    let mut rows = vec![row("a second assertion", second)];
    let first = disposition("disposition_a");
    let (status, created) = call(&host, "POST", dispositions, Some(first)).await;
    assert_eq!(status, 200, "{created}");
    for (name, path, body) in [
        ("a second disposition", dispositions, disposition("disposition_a")),
        ("a disposition of a decided proposal", dispositions, disposition("disposition_b")),
        ("an assertion of a disposed proposal", assertions, assertion("assertion_b")),
    ] {
        rows.push(row(name, call(&host, "POST", path, Some(body)).await));
    }
    assert_eq!(
        Value::Array(rows),
        json!([
            {"error":{"kind":"already_exists"},"row":"a second assertion","status":409},
            {"error":{"kind":"already_exists"},"row":"a second disposition","status":409},
            {"error":{"kind":"invalid_update",
                "message":"proposal already has an authoritative disposition"},
                "row":"a disposition of a decided proposal","status":400},
            {"error":{"kind":"invalid_update",
                "message":"a disposed proposal cannot re-enter assertion"},
                "row":"an assertion of a disposed proposal","status":400}
        ])
    );
}
