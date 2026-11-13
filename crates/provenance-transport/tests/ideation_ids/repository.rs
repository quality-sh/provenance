//! A repository with ideation records of each kind, the answers of the
//! ideation writes and the reads over it, and the hand edits that put an id
//! on two lines of its file.

use crate::support::ideation::evidence;
use crate::support::records::{allow_reviewer, Repository};
use crate::support::resource_http::{call, host};
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

/// The stored row of the assertion `assertion_a` of `proposal_a`.
pub fn assertion_row() -> Value {
    json!({"scope_id":"default", "id":"assertion_a", "proposal_id":"proposal_a",
        "synthesis_packet_id":"synthesis_evidence", "supporting_claim_ids":["claim_a"]})
}

pub fn state_path(repo: &Repository, name: &str) -> std::path::PathBuf {
    repo.dir
        .path()
        .join(".provenance/state")
        .join(IDEATION)
        .join(name)
}

/// A repository with the contribution `contribution_evidence`, the
/// synthesis packet `synthesis_evidence`, the proposal `proposal_a` with its
/// assertion `assertion_a` and its disposition `disposition_a`, and the
/// proposal `proposal_b` with no assertion. The result also gives the
/// answers of the reads over it.
///
/// No route creates a proposal that a packet qualifies, so one store call
/// creates `proposal_a` and its assertion, as a swarm landing does. That
/// call writes the landings file, and the proposal file and the assertion
/// file do not hold the two records.
pub async fn ideation_repository() -> (Repository, Vec<Value>) {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let writer = host(&repo, true);
    for (path, body) in evidence("proposal_a") {
        let (status, created) = call(&writer, "POST", path, Some(body)).await;
        assert_eq!(status, 200, "{path}: {created}");
    }
    let mut asserted = proposal("proposal_a", &["claim_a"])["data"].clone();
    asserted["scope_id"] = json!("default");
    repo.open()
        .store()
        .create_asserted_proposal(
            serde_json::from_value(asserted).unwrap(),
            serde_json::from_value(assertion_row()).unwrap(),
        )
        .unwrap();
    for (path, body) in [
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
        assert_eq!(
            read["body"]["meta"]["freshness_error"],
            Value::Null,
            "{read}"
        );
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
            meta["freshness_error"] = json!(fault);
            answer
        })
        .collect()
}

/// The parts of each read answer that a failed catch-up sets: the status,
/// the serial and the policy word of the stamp, and the `freshness_error`.
pub fn freshness_parts(answers: &[Value]) -> Vec<Value> {
    answers
        .iter()
        .map(|answer| {
            let meta = &answer["body"]["meta"];
            json!({"request":answer["request"], "status":answer["status"],
                "serial":meta["stamp"]["serial"], "policy":meta["stamp"]["policy"],
                "freshness_error":meta["freshness_error"]})
        })
        .collect()
}

/// The text of the file `name`, or no text when the file does not exist.
fn stored_text(repo: &Repository, name: &str) -> String {
    std::fs::read_to_string(state_path(repo, name)).unwrap_or_default()
}

/// Adds `rows` at the end of the file `name`. The result is the line of
/// each added row.
pub fn add_rows(repo: &Repository, name: &str, rows: &[String]) -> Vec<usize> {
    let text = stored_text(repo, name);
    let first = text.lines().count() + 1;
    std::fs::write(state_path(repo, name), format!("{text}{}", rows.concat())).unwrap();
    (first..first + rows.len()).collect()
}

/// Adds the row with the id `id` of the file `name` again at the end of the
/// file, with the id under the key `key`. The result is the line of the row
/// and the line of its copy.
pub fn copy_row_with_key(repo: &Repository, name: &str, id: &str, key: &str) -> (usize, usize) {
    let text = stored_text(repo, name);
    let pair = format!("\"id\":\"{id}\"");
    let keyed = format!("\"{key}\":\"{id}\"");
    let (first, row) = text
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains(&pair))
        .map(|(index, line)| (index + 1, line.replace(&pair, &keyed)))
        .unwrap();
    (first, add_rows(repo, name, &[format!("{row}\n")])[0])
}

/// Adds the row with the id `id` of the file `name` again at the end of the
/// file. The result is the line of the row and the line of its copy.
pub fn copy_row(repo: &Repository, name: &str, id: &str) -> (usize, usize) {
    copy_row_with_key(repo, name, id, "id")
}

/// Adds the row of `assertion_a` two times to the assertion file, as a
/// merge of two branches that each add it gives. The result is the two
/// lines.
pub fn merge_assertion_two_times(repo: &Repository) -> (usize, usize) {
    let row = format!("{}\n", assertion_row());
    let lines = add_rows(repo, "assertions.jsonl", &[row.clone(), row]);
    (lines[0], lines[1])
}

/// Adds a line that does not parse at the end of the file `name`. The
/// result is its line.
pub fn add_broken_line(repo: &Repository, name: &str) -> usize {
    add_rows(repo, name, &["{\"id\":\"broken\"\n".to_owned()])[0]
}
