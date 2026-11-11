//! Stored Discussions, Threads and Discussion parents that do not agree are
//! `invalid_state_file`: the answer names the file and the line of the record
//! at fault, and a message that names the records. Each test reaches the
//! check through a write, because a list read of hand-written state answers
//! `no_projection` first.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
    pub mod resource_http;
}

use provenance_macros::verifies;
use serde_json::{json, Value};
use support::records::Repository;
use support::resource_http::{call, host};

const DISCUSSIONS: &str = "scopes/default/threads/discussions.jsonl";
const THREADS: &str = "scopes/default/threads/threads.jsonl";
const REQUIREMENTS: &str = "scopes/default/requirements/req.jsonl";

fn path(repo: &Repository, shard: &str) -> std::path::PathBuf {
    repo.dir.path().join(".provenance/state").join(shard)
}

fn lines(repo: &Repository, shard: &str) -> Vec<String> {
    std::fs::read_to_string(path(repo, shard))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn write(repo: &Repository, shard: &str, lines: &[String]) {
    std::fs::write(path(repo, shard), lines.join("\n") + "\n").unwrap();
}

/// Adds a copy of the row with the id `id` at the end of the shard. Returns
/// the line of the copy.
fn store_again(repo: &Repository, shard: &str, field: &str, id: &str) -> usize {
    let mut rows = lines(repo, shard);
    let row = rows
        .iter()
        .find(|row| serde_json::from_str::<Value>(row).unwrap()[field] == id)
        .unwrap()
        .clone();
    rows.push(row);
    write(repo, shard, &rows);
    rows.len()
}

/// Changes the first row of the shard with `change`.
fn change_first(repo: &Repository, shard: &str, change: impl FnOnce(&mut Value)) {
    let mut rows = lines(repo, shard);
    let mut row: Value = serde_json::from_str(&rows[0]).unwrap();
    change(&mut row);
    rows[0] = row.to_string();
    write(repo, shard, &rows);
}

/// Starts a Discussion on `req_shared` and returns its id and Thread id.
async fn discussion(repo: &Repository) -> (String, String) {
    let (status, started) = call(
        &host(repo, true),
        "POST",
        "/requirements/req_shared/discussions",
        Some(json!({"data":{"actor":"reviewer","declared_by":null,"role":"user","body":"A concern."}})),
    )
    .await;
    assert_eq!(status, 200, "{started}");
    let row: Value = serde_json::from_str(&lines(repo, DISCUSSIONS)[0]).unwrap();
    (
        started["data"]["discussion_id"]
            .as_str()
            .unwrap()
            .to_owned(),
        row["thread_id"].as_str().unwrap().to_owned(),
    )
}

async fn start(repo: &Repository) -> (u16, Value) {
    call(
        &host(repo, true),
        "POST",
        "/requirements/req_shared/discussions",
        Some(json!({"data":{"actor":"reviewer","declared_by":null,"role":"user","body":"Again."}})),
    )
    .await
}

fn located(file: &str, line: usize, message: &str) -> Value {
    json!({"kind":"invalid_state_file", "file":file, "line":line, "message":message})
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_discussion_stored_again_is_located_at_the_second_line() {
    let repo = Repository::new("The shared graph is readable.");
    let (id, _) = discussion(&repo).await;
    let line = store_again(&repo, DISCUSSIONS, "discussion_id", &id);
    let (status, failure) = start(&repo).await;
    assert_eq!(status, 500, "{failure}");
    assert_eq!(
        failure["error"],
        located(
            DISCUSSIONS,
            line,
            &format!("the discussion id {id} is stored again")
        )
    );
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_discussion_that_names_a_missing_thread_or_another_scope_is_located() {
    let repo = Repository::new("The shared graph is readable.");
    discussion(&repo).await;
    let original = lines(&repo, DISCUSSIONS);
    change_first(&repo, DISCUSSIONS, |row| {
        row["thread_id"] = json!("thread_missing");
    });
    let (status, missing) = start(&repo).await;
    write(&repo, DISCUSSIONS, &original);
    change_first(&repo, DISCUSSIONS, |row| {
        row["scope_id"] = json!("other");
    });
    let (other_status, other) = start(&repo).await;
    assert_eq!((status, other_status), (500, 500), "{missing} {other}");
    assert_eq!(
        [&missing["error"], &other["error"]],
        [
            &located(
                DISCUSSIONS,
                1,
                "the discussion names thread thread_missing, which does not exist"
            ),
            &located(
                DISCUSSIONS,
                1,
                "the discussion names scope other, not the scope of its file"
            ),
        ]
    );
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_thread_stored_again_is_located_at_the_second_line() {
    let repo = Repository::new("The shared graph is readable.");
    let (_, thread) = discussion(&repo).await;
    let line = store_again(&repo, THREADS, "id", &thread);
    let (status, failure) = start(&repo).await;
    assert_eq!(status, 500, "{failure}");
    assert_eq!(
        failure["error"],
        located(THREADS, line, &format!("thread {thread} is stored again"))
    );
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn an_origin_thread_stored_again_is_located_at_the_second_line() {
    let repo = Repository::new("The shared graph is readable.");
    let (_, thread) = discussion(&repo).await;
    let line = store_again(&repo, THREADS, "id", &thread);
    let (status, failure) = call(
        &host(&repo, true),
        "POST",
        "/requirements",
        Some(json!({"data":{
            "actor":"reviewer", "id":"req_from_thread", "statement":"The system stores each record.",
            "status":"discovery", "depends_on":[], "supersedes":[], "origin_thread":thread
        }})),
    )
    .await;
    assert_eq!(status, 500, "{failure}");
    assert_eq!(
        failure["error"],
        located(THREADS, line, "origin Thread identity is not unique")
    );
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_discussion_parent_stored_again_is_located_at_the_second_line() {
    let repo = Repository::new("The shared graph is readable.");
    let line = store_again(&repo, REQUIREMENTS, "id", "req_shared");
    let (status, failure) = start(&repo).await;
    assert_eq!(status, 500, "{failure}");
    assert_eq!(
        failure["error"],
        located(
            REQUIREMENTS,
            line,
            "Discussion parent req_shared is stored again"
        )
    );
}
