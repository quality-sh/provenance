//! A proposal, assertion or disposition id is immutable. When a hand edit
//! or a branch merge puts one id on two lines of its file, each ideation
//! write refuses with `invalid_state_file` at the second line. A resource
//! read loads the proposals and the dispositions of the review cycle, and
//! refuses in the same way. A list read answers from the projection and
//! gives the same line as the fault of its catch-up.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
    pub mod resource_http;
}

use provenance_macros::verifies;
use serde_json::{json, Value};
use support::records::{allow_reviewer, Repository};
use support::resource_http::{call, host};

const IDEATION: &str = "scopes/default/ideation";
const TARGET: &str = r#"{"artifact_type":"requirement","artifact_id":"req_shared"}"#;

fn proposal(id: &str, claims: &[&str]) -> Value {
    json!({"data":{
        "id":id, "proposal_key":id, "proposal_type":"requirement_candidate",
        "title":"Candidate", "summary":"Clarify the shared requirement.",
        "traceability":{"target":serde_json::from_str::<Value>(TARGET).unwrap(),
            "source_ids":[], "evidence_references":[], "supporting_claim_ids":claims},
        "builds_on":[], "promotion_state":"proposed"
    }})
}

/// A contribution with the claim `claim_a` and a synthesis packet that
/// suggests `proposal_a`, as a merge of a branch that adds them gives.
fn merge_evidence(repo: &Repository) {
    let target: Value = serde_json::from_str(TARGET).unwrap();
    let contribution = json!({
        "scope_id":"default", "id":"contribution_evidence", "target":target,
        "participant_slot":"reviewer", "stance":"support", "strongest_finding":"Observed",
        "evidence_references":[{"reference_id":"evidence_a","evidence_type":"source","summary":"Pinned"}],
        "material_claims":[{"claim_id":"claim_a","statement":"Observed",
            "evidence_type":"source","evidence_reference_ids":["evidence_a"]}],
        "risks":[], "objections":[], "challenges":[], "suggested_artifact_changes":[],
        "unsupported_recommendations":[], "uncertainty":{"level":"low","rationale":"Direct"},
        "open_questions":[]
    });
    let packet = json!({
        "scope_id":"default", "id":"synthesis_evidence", "target":target,
        "summary":"Adjudicated", "consensus":[], "contested_claims":[],
        "minority_objections":[], "evidence_gaps":[], "unsupported_speculation":[],
        "open_questions":[],
        "suggested_artifacts":[{"proposal_id":"proposal_a","proposal_key":"proposal_a",
            "proposal_type":"requirement_candidate","summary":"Candidate",
            "origin_participant_slots":["reviewer"]}],
        "required_human_decisions":[]
    });
    for (name, record) in [
        ("contributions.jsonl", contribution),
        ("synthesis_packets.jsonl", packet),
    ] {
        let path = state_path(repo, name);
        std::fs::write(path, format!("{record}\n")).unwrap();
    }
}

fn state_path(repo: &Repository, name: &str) -> std::path::PathBuf {
    repo.dir
        .path()
        .join(".provenance/state")
        .join(IDEATION)
        .join(name)
}

/// A repository with one row in each of the proposal, assertion and
/// disposition files, and the proposal `proposal_b` with no assertion.
async fn ideation_repository() -> Repository {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let writer = host(&repo, true);
    let (status, created) = call(
        &writer,
        "POST",
        "/proposals",
        Some(proposal("proposal_a", &["claim_a"])),
    )
    .await;
    assert_eq!(status, 200, "{created}");
    merge_evidence(&repo);
    for (path, body) in [
        (
            "/proposals/proposal_a/assertions",
            json!({"data":{"id":"assertion_a", "synthesis_packet_id":"synthesis_evidence",
                "supporting_claim_ids":["claim_a"]}}),
        ),
        (
            "/proposals/proposal_a/dispositions",
            json!({"data":{"id":"disposition_a", "decision":"rejected", "rationale":"Reviewed",
                "actor":{"identity_type":"human","id":"reviewer"}}}),
        ),
        ("/proposals", proposal("proposal_b", &[])),
    ] {
        let (status, created) = call(&writer, "POST", path, Some(body)).await;
        assert_eq!(status, 200, "{path}: {created}");
    }
    for path in READS {
        let (status, listed) = call(&writer, "GET", path, None).await;
        assert_eq!(status, 200, "{path}: {listed}");
        assert_eq!(listed["meta"]["freshness_error"], Value::Null, "{path}");
    }
    repo
}

const READS: [&str; 4] = [
    "/requirements/req_shared",
    "/proposals",
    "/assertions",
    "/dispositions",
];

/// The status and the error object of each read and each ideation write.
async fn answers(repo: &Repository) -> Vec<Value> {
    let requests = [
        ("POST", "/proposals", Some(proposal("proposal_c", &[]))),
        (
            "POST",
            "/proposals/proposal_b/assertions",
            Some(
                json!({"data":{"id":"assertion_b", "synthesis_packet_id":"synthesis_evidence",
                "supporting_claim_ids":[]}}),
            ),
        ),
        (
            "POST",
            "/proposals/proposal_b/dispositions",
            Some(json!({"data":{"id":"disposition_b", "decision":"deferred",
                "rationale":"Later", "actor":{"identity_type":"human","id":"reviewer"}}})),
        ),
    ];
    let reads = READS.map(|path| ("GET", path, None));
    let writer = host(repo, true);
    let mut answers = Vec::new();
    for (method, path, body) in requests.into_iter().chain(reads) {
        let (status, answer) = call(&writer, method, path, body).await;
        answers.push(
            json!({"request":format!("{method} {path}"), "status":status,
            "error":answer["error"], "freshness_error":answer["meta"]["freshness_error"]}),
        );
    }
    answers
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn an_ideation_id_on_two_lines_is_located_at_the_second_line() {
    for (name, kind, id) in [
        ("proposal_cards.jsonl", "proposal", "proposal_a"),
        ("assertions.jsonl", "assertion", "assertion_a"),
        ("dispositions.jsonl", "disposition", "disposition_a"),
    ] {
        let repo = ideation_repository().await;
        let path = state_path(&repo, name);
        let text = std::fs::read_to_string(&path).unwrap();
        let row = text
            .lines()
            .find(|line| line.contains(&format!("\"id\":\"{id}\"")))
            .unwrap()
            .to_owned();
        let second = text.lines().count() + 1;
        std::fs::write(&path, format!("{text}{row}\n")).unwrap();
        let file = format!("{IDEATION}/{name}");
        let message = format!("duplicate immutable {kind} id {id}");
        let refusal = json!({"kind":"invalid_state_file", "file":file, "line":second,
            "message":message});
        let answers = answers(&repo).await;
        let expected = answers
            .iter()
            .map(|answer| {
                let request = answer["request"].as_str().unwrap();
                let loads_file = request == "GET /requirements/req_shared" && kind != "assertion";
                if request.starts_with("GET ") && !loads_file {
                    json!({"request":request, "status":200, "error":null,
                        "freshness_error":format!("{file} line {second}: {message}")})
                } else {
                    json!({"request":request, "status":500, "error":refusal,
                        "freshness_error":null})
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(answers, expected, "{name}");
    }
}
