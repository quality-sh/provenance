use provenance_http_client::{Error, HttpClient};
use serde_json::{json, Value};
use std::io::Write;

fn request(context: &Value, inner: &Value) -> Value {
    json!({"context": context, "request": inner})
}

fn ideation_dir(fixture: &Value) -> std::path::PathBuf {
    std::path::Path::new(fixture["root"].as_str().unwrap())
        .join(".provenance/state/scopes/default/ideation")
}

/// Seed the contribution and synthesis packet the supported assertion rests
/// on, appended beside any earlier fixture records.
fn seed_assertion_evidence(fixture: &Value) {
    let contribution = json!({
        "schema_version": 2, "scope_id": "default", "id": "contribution_rs",
        "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"},
        "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Observed",
        "evidence_references": [{"reference_id": "evidence_rs", "evidence_type": "source", "summary": "Pinned"}],
        "material_claims": [{"claim_id": "claim_rs", "statement": "Observed", "evidence_type": "source", "evidence_reference_ids": ["evidence_rs"]}],
        "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
        "unsupported_recommendations": [], "uncertainty": {"level": "low", "rationale": "Direct"},
        "open_questions": []
    });
    let packet = json!({
        "schema_version": 2, "scope_id": "default", "id": "synthesis_rs",
        "target": {"artifact_type": "requirement", "artifact_id": "req_overtime"},
        "summary": "Adjudicated",
        "consensus": [], "contested_claims": [], "minority_objections": [],
        "evidence_gaps": [], "unsupported_speculation": [], "open_questions": [],
        "suggested_artifacts": [{"proposal_id": "proposal_rs", "proposal_key": "overtime",
            "proposal_type": "requirement_candidate", "summary": "Candidate",
            "origin_participant_slots": ["reviewer"]}],
        "required_human_decisions": []
    });
    let dir = ideation_dir(fixture);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, record) in [
        ("contributions.jsonl", contribution),
        ("synthesis_packets.jsonl", packet),
    ] {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(name))
            .unwrap();
        writeln!(file, "{record}").unwrap();
    }
}

fn mine<'a>(rows: &'a Value, id: &str) -> &'a Value {
    rows.as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("no {id} in {rows}"))
}

#[tokio::test]
#[ignore = "run through node tools/operation-codegen/test-clients.mjs ideation"]
async fn generated_ideation_methods_preserve_records_and_refusals() {
    let fixture: Value =
        serde_json::from_str(&std::env::var("PROVENANCE_RECORDS_FIXTURE").unwrap()).unwrap();
    let client =
        HttpClient::connect_with_bearer(fixture["url"].as_str().unwrap(), "fixture-secret")
            .await
            .unwrap();
    let context = json!({"repository":"fixture","scope":"default"});
    let proposal = json!({"scope_id":"default","id":"proposal_rs","proposal_key":"overtime",
        "proposal_type":"requirement_candidate","title":"Overtime","summary":"Clarify overtime.",
        "traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_overtime"},
            "source_ids":[],"evidence_references":[],"supporting_claim_ids":["claim_rs"]},
        "builds_on":[],"promotion_state":"proposed"});
    let created = client
        .create_proposal(&serde_json::from_value(request(&context, &proposal)).unwrap())
        .await
        .unwrap();
    assert_eq!(json!(created)["promotion_state"], "proposed");
    seed_assertion_evidence(&fixture);
    let assertion = client
        .create_assertion(
            &serde_json::from_value(request(
                &context,
                &json!({
            "scope_id":"default","id":"assertion_rs","proposal_id":"proposal_rs",
            "synthesis_packet_id":"synthesis_rs","supporting_claim_ids":["claim_rs"]}),
            ))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(json!(assertion)["proposal_id"], "proposal_rs");
    let listed = client
        .list_proposals(&serde_json::from_value(request(&context, &Value::Null)).unwrap())
        .await
        .unwrap();
    assert_eq!(
        mine(&json!(listed), "proposal_rs")["promotion_state"],
        "asserted"
    );
    let assertions = client
        .list_assertions(&serde_json::from_value(request(&context, &Value::Null)).unwrap())
        .await
        .unwrap();
    assert!(json!(assertions)
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == "assertion_rs"));
    let disposition = client
        .create_disposition(
            &serde_json::from_value(request(
                &context,
                &json!({
            "scope_id":"default","id":"disposition_rs","proposal_id":"proposal_rs",
            "decision":"rejected","rationale":"Reviewed",
            "actor":{"identity_type":"human","id":"reviewer"}}),
            ))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(json!(disposition)["decision"], "rejected");
    let settled = client
        .list_proposals(&serde_json::from_value(request(&context, &Value::Null)).unwrap())
        .await
        .unwrap();
    assert_eq!(
        mine(&json!(settled), "proposal_rs")["promotion_state"],
        "rejected"
    );
    let dispositions = client
        .list_dispositions(&serde_json::from_value(request(&context, &Value::Null)).unwrap())
        .await
        .unwrap();
    assert!(json!(dispositions)
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == "disposition_rs"));
    refusal_outcomes(&client, &context).await;
}

/// Unclassified pre-publication refusals stay uncertain for a mutating call:
/// the client cannot tell them from a lost mutation response. A typed refusal
/// keeps its status and kind.
async fn refusal_outcomes(client: &HttpClient, context: &Value) {
    for inner in [
        json!({"scope_id":"default","id":"disposition_second","proposal_id":"proposal_rs",
            "decision":"rejected","rationale":"Reviewed",
            "actor":{"identity_type":"human","id":"reviewer"}}),
        json!({"scope_id":"default","id":"disposition_forged","proposal_id":"proposal_rs",
            "decision":"rejected","rationale":"Reviewed",
            "actor":{"identity_type":"human","id":"forged-reviewer"}}),
    ] {
        match client
            .create_disposition(&serde_json::from_value(request(context, &inner)).unwrap())
            .await
        {
            Err(Error::UncertainWrite { failure, .. }) => {
                assert_eq!(json!(failure.unwrap())["error"]["kind"], "write_failed");
            }
            result => panic!("expected uncertain write, got {result:?}"),
        }
    }
    match client
        .create_disposition(
            &serde_json::from_value(request(
                context,
                &json!({
            "scope_id":"other","id":"disposition_other","proposal_id":"proposal_rs",
            "decision":"rejected","rationale":"Reviewed",
            "actor":{"identity_type":"human","id":"reviewer"}}),
            ))
            .unwrap(),
        )
        .await
    {
        Err(Error::Operation { status, failure }) => {
            assert_eq!(json!(failure)["error"]["kind"], "scope_mismatch");
            assert_eq!(status, 400);
        }
        result => panic!("expected typed refusal, got {result:?}"),
    }
}
