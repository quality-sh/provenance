//! A repository with one proposal, assertion and disposition, and the
//! answers of the ideation writes and the reads over it.
#![allow(dead_code)]

use super::ideation::evidence;
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

/// The evidence of `proposal_a` in the contribution and synthesis files, as
/// a merge of a branch that adds them gives. No route adds a packet that
/// qualifies a proposal with no assertion.
pub fn merge_evidence(repo: &Repository) {
    let names = ["contributions.jsonl", "synthesis_packets.jsonl"];
    for ((_, body), name) in evidence("proposal_a").into_iter().zip(names) {
        let mut record = body["data"].clone();
        record["scope_id"] = json!("default");
        std::fs::write(state_path(repo, name), format!("{record}\n")).unwrap();
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
/// disposition files, and the proposal `proposal_b` with no assertion. The
/// result also gives the answers of the reads over it.
pub async fn ideation_repository() -> (Repository, Vec<Value>) {
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
    let reads = read_answers(&repo).await;
    for read in &reads {
        assert_eq!(read["status"], 200, "{read}");
        assert_eq!(read["body"]["meta"]["freshness_error"], Value::Null, "{read}");
    }
    (repo, reads)
}

pub const READS: [&str; 4] = [
    "/requirements/req_shared",
    "/proposals",
    "/assertions",
    "/dispositions",
];

/// The answer of each request: the request, the status and the body.
async fn answer_all(repo: &Repository, requests: Vec<(&str, &str, Option<Value>)>) -> Vec<Value> {
    let writer = host(repo, true);
    let mut answers = Vec::new();
    for (method, path, body) in requests {
        let (status, body) = call(&writer, method, path, body).await;
        answers.push(json!({"request":format!("{method} {path}"), "status":status, "body":body}));
    }
    answers
}

pub const WRITES: [&str; 3] = [
    "POST /proposals",
    "POST /proposals/proposal_b/assertions",
    "POST /proposals/proposal_b/dispositions",
];

/// The answers of the three ideation writes, in the order of [`WRITES`].
pub async fn write_answers(repo: &Repository) -> Vec<Value> {
    let requests = vec![
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
    answer_all(repo, requests).await
}

/// The answers of the reads, in the order of [`READS`].
pub async fn read_answers(repo: &Repository) -> Vec<Value> {
    answer_all(repo, READS.map(|path| ("GET", path, None)).to_vec()).await
}

/// The answers of the writes and then of the reads.
pub async fn answers(repo: &Repository) -> Vec<Value> {
    let mut answers = write_answers(repo).await;
    answers.extend(read_answers(repo).await);
    answers
}

/// The answer of `request` when it refuses with the error object `error`.
pub fn refused(request: &str, error: &Value) -> Value {
    json!({"request":request, "status":500, "body":{"error":error, "meta":{}}})
}

/// The answers `healthy` of the reads, at the stored serial after a catch-up
/// that fails with the text `fault`.
pub fn at_stored(healthy: &[Value], fault: &str) -> Vec<Value> {
    healthy
        .iter()
        .map(|answer| {
            let mut answer = answer.clone();
            let meta = &mut answer["body"]["meta"];
            meta["stamp"]["policy"] = json!("catch_up_failed");
            meta["freshness_cause"] = json!("catch_up_failed");
            meta["freshness_error"] = json!(fault);
            answer
        })
        .collect()
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
