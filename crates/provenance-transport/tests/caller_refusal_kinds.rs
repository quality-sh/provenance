//! A refusal that the input of a caller causes has a declared kind and a
//! status that is not 500, for writes that go through the review store.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
    pub mod resource_http;
}

use provenance_macros::verifies;
use serde_json::{json, Value};
use support::records::Repository;
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
