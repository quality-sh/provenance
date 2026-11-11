//! A repository with one proposal, assertion and disposition, and the
//! answers of the ideation writes and the reads over it.
#![allow(dead_code)]

use super::records::{allow_reviewer, Repository};
use super::resource_http::{call, host};
use serde_json::{json, Value};

pub const IDEATION: &str = "scopes/default/ideation";
const TARGET: &str = r#"{"artifact_type":"requirement","artifact_id":"req_shared"}"#;

pub fn proposal(id: &str, claims: &[&str]) -> Value {
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
pub fn merge_evidence(repo: &Repository) {
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

pub fn state_path(repo: &Repository, name: &str) -> std::path::PathBuf {
    repo.dir
        .path()
        .join(".provenance/state")
        .join(IDEATION)
        .join(name)
}

/// A repository with one row in each of the proposal, assertion and
/// disposition files, and the proposal `proposal_b` with no assertion.
pub async fn ideation_repository() -> Repository {
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

pub const READS: [&str; 4] = [
    "/requirements/req_shared",
    "/proposals",
    "/assertions",
    "/dispositions",
];

/// The status and the error object of each read and each ideation write.
pub async fn answers(repo: &Repository) -> Vec<Value> {
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

/// Adds the row with the id `id` of the file `name` again at the end of the
/// file. The result is the line of the row and the line of its copy.
pub fn copy_row(repo: &Repository, name: &str, id: &str) -> (usize, usize) {
    let path = state_path(repo, name);
    let text = std::fs::read_to_string(&path).unwrap();
    let (first, row) = text
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains(&format!("\"id\":\"{id}\"")))
        .map(|(index, line)| (index + 1, line.to_owned()))
        .unwrap();
    std::fs::write(&path, format!("{text}{row}\n")).unwrap();
    (first, text.lines().count() + 1)
}

/// Adds a line that does not parse at the end of the file `name`. The
/// result is its line.
pub fn add_broken_line(repo: &Repository, name: &str) -> usize {
    let path = state_path(repo, name);
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{text}{{\"id\":\"broken\"\n")).unwrap();
    text.lines().count() + 1
}
