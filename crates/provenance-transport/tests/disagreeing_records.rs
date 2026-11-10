//! Stored records that do not agree are `invalid_state_file`: the answer
//! names the file and the line of the record at fault, and a message that
//! names the records (8yzq).
#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
    pub mod resource_http;
}

use provenance_macros::verifies;
use serde_json::json;
use support::records::Repository;
use support::resource_http::{call, host};

const WITHDRAWALS: &str = "scopes/default/ideation/withdrawals.jsonl";

fn withdrawal(id: &str, proposal: &str) -> String {
    json!({"scope_id":"default","id":id,"proposal_id":proposal,"actor":"reviewer","reason":null})
        .to_string()
}

fn write_withdrawals(repo: &Repository, lines: &[String]) {
    let path = repo.dir.path().join(".provenance/state").join(WITHDRAWALS);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, lines.join("\n") + "\n").unwrap();
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_withdrawal_that_names_no_submission_is_located() {
    let repo = Repository::new("The shared graph is readable.");
    write_withdrawals(&repo, &[withdrawal("withdrawal_lost", "proposal_missing")]);
    let host = host(&repo, false);
    let (status, failure) = call(&host, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 500, "{failure}");
    assert_eq!(
        failure["error"],
        json!({"kind":"invalid_state_file", "file":WITHDRAWALS, "line":1,
            "message":"withdrawal withdrawal_lost names no review submission: proposal_missing"}),
    );
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_submission_withdrawn_two_times_is_located_at_the_second_withdrawal() {
    let repo = Repository::new("The shared graph is readable.");
    support::records::allow_reviewer(&repo);
    let writer = host(&repo, true);
    let (status, read) = call(&writer, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 200, "{read}");
    let etag = read["data"]["edit"]["etag"].as_str().unwrap().to_owned();
    let (status, saved) = support::resource_http::call_with_headers(
        &writer,
        "PATCH",
        "/requirements/req_shared",
        Some(json!({"data":{"actor":"agent", "description":"Enrolled."}})),
        &[("if-match", &etag)],
    )
    .await;
    assert_eq!(status, 200, "{saved}");
    let proposal = saved["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned();
    write_withdrawals(
        &repo,
        &[
            withdrawal("withdrawal_a", &proposal),
            withdrawal("withdrawal_b", &proposal),
        ],
    );
    let (status, failure) =
        call(&host(&repo, false), "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 500, "{failure}");
    assert_eq!(
        failure["error"],
        json!({"kind":"invalid_state_file", "file":WITHDRAWALS, "line":2,
        "message":format!(
            "review submission {proposal} is withdrawn two times: withdrawal_a and withdrawal_b"
        )}),
    );
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_requirement_stored_on_two_lines_is_located_at_the_second_line() {
    const REQUIREMENTS: &str = "scopes/default/requirements/req.jsonl";
    let repo = Repository::new("The shared graph is readable.");
    let path = repo.dir.path().join(".provenance/state").join(REQUIREMENTS);
    let text = std::fs::read_to_string(&path).unwrap();
    let (first, line) = text
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains("\"id\":\"req_shared\""))
        .map(|(index, line)| (index + 1, line.to_owned()))
        .unwrap();
    let second = text.lines().count() + 1;
    std::fs::write(&path, format!("{text}{line}\n")).unwrap();
    let (status, failure) = support::resource_http::call_with_headers(
        &host(&repo, true),
        "PATCH",
        "/requirements/req_shared",
        Some(json!({"data":{"actor":"reviewer", "description":"Edited."}})),
        &[("if-match", "\"stale\"")],
    )
    .await;
    assert_eq!(status, 500, "{failure}");
    assert_eq!(
        failure["error"],
        json!({"kind":"invalid_state_file", "file":REQUIREMENTS, "line":second,
            "message":format!("requirement req_shared is stored again; line {first} stores it first")}),
    );
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_message_id_on_two_lines_is_located_at_the_second_line() {
    let repo = Repository::new("The shared graph is readable.");
    let (status, started) = call(
        &host(&repo, true),
        "POST",
        "/requirements/req_shared/discussions",
        Some(json!({"data":{"actor":"reviewer","declared_by":null,"role":"user","body":"A concern."}})),
    )
    .await;
    assert_eq!(status, 200, "{started}");
    let threads = repo
        .dir
        .path()
        .join(".provenance/state/scopes/default/threads");
    let shard = std::fs::read_dir(&threads)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .find(|name| name.len() == "2026-11.jsonl".len() && name.as_bytes()[4] == b'-')
        .unwrap();
    let path = threads.join(&shard);
    let text = std::fs::read_to_string(&path).unwrap();
    let line = text.lines().next().unwrap().to_owned();
    let id = serde_json::from_str::<serde_json::Value>(&line).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    std::fs::write(&path, format!("{line}\n{text}")).unwrap();
    let (status, failure) = call(&host(&repo, false), "GET", "/messages", None).await;
    assert_eq!(status, 500, "{failure}");
    let file = format!("scopes/default/threads/{shard}");
    assert_eq!(
        failure["error"],
        json!({"kind":"invalid_state_file", "file":file, "line":2,
        "message":format!(
            "Message ID {id} appears more than once: {file}:1, {file}:2; \
             see docs/cli.md#repairing-a-duplicate-message-id"
        )}),
    );
}
