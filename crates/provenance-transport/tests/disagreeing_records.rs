//! Stored records that do not agree are `invalid_state_file`: the answer
//! names the file and the line of the record at fault, and a message that
//! names the records.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_core::protocol::read_failure::STATE_CORRECTIONS;
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

/// The record read over a stored revision refuses with the Withdrawal.
#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_withdrawal_that_names_no_submission_is_located() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, false);
    let (status, read) = call(&host, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 200, "{read}");
    write_withdrawals(&repo, &[withdrawal("withdrawal_lost", "proposal_missing")]);
    let (status, failure) = call(&host, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 500, "{failure}");
    assert_eq!(
        failure["error"],
        json!({"kind":"invalid_state_file", "file":WITHDRAWALS, "line":1,
            "message":"withdrawal withdrawal_lost names no review submission: proposal_missing",
            "hint":"To repair, remove ideation/withdrawals.jsonl line 1, then commit."}),
    );
}

/// With no stored revision, the catch-up that builds the read database
/// refuses the Withdrawal, so the record read refuses with `no_projection`.
/// The refusal gives the Withdrawal as the cause and names
/// `provenance materialize`.
#[tokio::test]
#[verifies("rule_no_revision_refuses_and_names_materialize", examples)]
async fn a_withdrawal_that_names_no_submission_with_no_stored_revision_refuses() {
    let repo = Repository::new("The shared graph is readable.");
    write_withdrawals(&repo, &[withdrawal("withdrawal_lost", "proposal_missing")]);
    let host = host(&repo, false);
    let (status, failure) = call(&host, "GET", "/requirements/req_shared", None).await;
    assert_eq!(
        (status, failure),
        (
            409,
            json!({"error":{"kind":"no_projection",
                "message":format!("catch-up failed: {WITHDRAWALS} line 1: \
                    withdrawal withdrawal_lost names no review submission: proposal_missing; \
                    To repair, remove ideation/withdrawals.jsonl line 1, then commit."),
                "hint":"run `provenance materialize`"},"meta":{}})
        )
    );
}

/// The pending review submission of `req_shared`, which an edit opens.
async fn pending_submission(repo: &Repository) -> String {
    support::records::allow_reviewer(repo);
    let writer = host(repo, true);
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
    saved["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// The public surface writes each Withdrawal one time with a new id, so
/// only a hand edit of the state file puts one id on two lines. Two equal
/// lines read as one Withdrawal.
#[tokio::test]
#[verifies("rule_equal_review_row_lines_read_as_one", examples)]
async fn a_withdrawal_id_on_two_equal_lines_reads_as_one() {
    let repo = Repository::new("The shared graph is readable.");
    let proposal = pending_submission(&repo).await;
    let line = withdrawal("withdrawal_a", &proposal);
    write_withdrawals(&repo, &[line.clone(), line]);
    let (status, read) = call(&host(&repo, false), "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 200, "{read}");
    assert_eq!(read["data"]["decision"]["withdrawn"], json!([proposal]));
    assert_eq!(read["data"]["decision"]["pending"], serde_json::Value::Null);
}

/// Two lines of one Withdrawal id with different contents refuse the record
/// that the Withdrawal names, at the second line.
#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_withdrawal_id_on_two_lines_is_located_at_the_second_line() {
    let repo = Repository::new("The shared graph is readable.");
    let proposal = pending_submission(&repo).await;
    let edited = json!({"scope_id":"default", "id":"withdrawal_a", "proposal_id":proposal,
        "actor":"agent", "reason":"Edited."});
    write_withdrawals(
        &repo,
        &[withdrawal("withdrawal_a", &proposal), edited.to_string()],
    );
    let (status, failure) =
        call(&host(&repo, false), "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 500, "{failure}");
    assert_eq!(
        failure["error"],
        json!({"kind":"invalid_state_file", "file":WITHDRAWALS, "line":2,
            "message":"withdrawal withdrawal_a is on lines 1 and 2 of ideation/withdrawals.jsonl \
                with different contents",
            "hint":"To repair, keep one of these lines and remove the others, then commit."}),
    );
}

/// Two Withdrawals of one submission are not a fault, so the refusal names
/// the later Withdrawal that names no review submission, at its own line.
#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_withdrawal_that_names_no_submission_after_two_withdrawals_is_located() {
    let repo = Repository::new("The shared graph is readable.");
    let proposal = pending_submission(&repo).await;
    write_withdrawals(
        &repo,
        &[
            withdrawal("withdrawal_a", &proposal),
            withdrawal("withdrawal_b", &proposal),
            withdrawal("withdrawal_lost", "proposal_missing"),
        ],
    );
    let (status, failure) =
        call(&host(&repo, false), "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 500, "{failure}");
    assert_eq!(
        failure["error"],
        json!({"kind":"invalid_state_file", "file":WITHDRAWALS, "line":3,
            "message":"withdrawal withdrawal_lost names no review submission: proposal_missing",
            "hint":"To repair, remove ideation/withdrawals.jsonl line 3, then commit."}),
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
            "message":format!("requirement req_shared is stored again; line {first} stores it first"),
            "hint":STATE_CORRECTIONS}),
    );
}

/// The refusal of a duplicate Message ID has a correction of its own: its
/// hint names the hand repair and not the two corrections of the state.
#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
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
    let discussion = started["data"]["discussion_id"].as_str().unwrap();
    let (status, failure) = support::resource_http::call_with_headers(
        &host(&repo, true),
        "POST",
        &format!("/requirements/req_shared/discussions/{discussion}/messages"),
        Some(
            json!({"data":{"actor":"agent","declared_by":null,"role":"assistant","body":"Reply."}}),
        ),
        &[("if-match", "\"1\"")],
    )
    .await;
    assert_eq!(status, 500, "{failure}");
    let file = format!("scopes/default/threads/{shard}");
    assert_eq!(
        failure["error"],
        json!({"kind":"invalid_state_file", "file":file, "line":2,
        "message":format!("Message ID {id} appears more than once: {file}:1, {file}:2"),
        "hint":"see docs/cli.md#repairing-a-duplicate-message-id"}),
    );
}

/// Two decisions with different results on the submission of `req_shared`
/// refuse its read, and the read of a different record answers. The test
/// writes the two decision lines by hand, as a merge of two branches
/// stores them.
#[tokio::test]
#[verifies("rule_review_row_refusal_stays_with_record", examples)]
async fn decisions_with_different_results_refuse_only_their_record() {
    const DISPOSITIONS: &str = "scopes/default/ideation/dispositions.jsonl";
    let repo = Repository::new("The shared graph is readable.");
    let proposal = pending_submission(&repo).await;
    let (status, created) = call(
        &host(&repo, true),
        "POST",
        "/requirements",
        Some(json!({"data":{"actor":"agent", "id":"req_other",
            "statement":"The other record is readable.", "status":"active",
            "depends_on":[], "supersedes":[]}})),
    )
    .await;
    assert_eq!(status, 200, "{created}");
    let decision = |id: &str, result: &str| {
        json!({"scope_id":"default", "id":id, "proposal_id":proposal, "decision":result,
            "rationale":"By hand.", "actor":{"identity_type":"human","id":"reviewer"}})
        .to_string()
    };
    let path = repo.dir.path().join(".provenance/state").join(DISPOSITIONS);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let lines = [
        decision("decision_a", "deferred"),
        decision("decision_b", "rejected"),
    ];
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    let reader = host(&repo, false);
    let (status, failure) = call(&reader, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 500, "{failure}");
    assert_eq!(
        failure["error"],
        json!({"kind":"invalid_state_file", "file":DISPOSITIONS, "line":2,
            "message":format!("review submission {proposal} of requirement req_shared has \
                decisions with different results in ideation/dispositions.jsonl: deferred \
                decision_a at line 1 and rejected decision_b at line 2"),
            "hint":"To repair, remove each of these lines that has a result that you do not keep, \
                then commit."}),
    );
    let (status, other) = call(&reader, "GET", "/requirements/req_other", None).await;
    assert_eq!(status, 200, "{other}");
}

/// A hand edit copies the line of the submission of `req_shared` and changes
/// its summary, and a decision names the submission. The decision is a part
/// of the fault of `req_shared`: only that record refuses, and the list
/// reads, a different record, its document and a new record answer.
#[tokio::test]
#[verifies("rule_review_row_refusal_stays_with_record", examples)]
async fn a_decision_on_a_submission_on_lines_that_disagree_refuses_only_its_record() {
    const PROPOSALS: &str = "scopes/default/ideation/proposal_cards.jsonl";
    const DISPOSITIONS: &str = "scopes/default/ideation/dispositions.jsonl";
    let repo = Repository::new("The shared graph is readable.");
    let proposal = pending_submission(&repo).await;
    let state = repo.dir.path().join(".provenance/state");
    let text = std::fs::read_to_string(state.join(PROPOSALS)).unwrap();
    let named = format!("\"id\":\"{proposal}\"");
    let line = text.lines().find(|line| line.contains(&named)).unwrap();
    let mut changed: serde_json::Value = serde_json::from_str(line).unwrap();
    changed["summary"] = json!("Changed by hand.");
    std::fs::write(state.join(PROPOSALS), format!("{text}{changed}\n")).unwrap();
    let decision = json!({"scope_id":"default", "id":"decision_a", "proposal_id":proposal,
        "decision":"rejected", "rationale":"By hand.",
        "actor":{"identity_type":"human","id":"reviewer"}});
    std::fs::write(state.join(DISPOSITIONS), format!("{decision}\n")).unwrap();
    let reader = host(&repo, true);
    let (status, created) = call(
        &reader,
        "POST",
        "/requirements",
        Some(json!({"data":{"actor":"agent", "id":"req_other",
            "statement":"The other record is readable.", "status":"active",
            "depends_on":[], "supersedes":[]}})),
    )
    .await;
    assert_eq!(status, 200, "{created}");
    for path in [
        "/requirements/req_other",
        "/requirements/req_other/document",
        "/proposals",
        "/dispositions",
    ] {
        let (status, answer) = call(&reader, "GET", path, None).await;
        assert_eq!(status, 200, "{path} {answer}");
        assert_eq!(
            answer["meta"]["freshness_error"],
            json!(null),
            "{path} {answer}"
        );
    }
    // The write of req_other wrote the file again, so the lines moved.
    let text = std::fs::read_to_string(state.join(PROPOSALS)).unwrap();
    let holding = text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(&named))
        .map(|(index, _)| index + 1)
        .collect::<Vec<_>>();
    let (first, second) = (holding[0], holding[1]);
    let (status, failure) = call(&reader, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 500, "{failure}");
    assert_eq!(
        failure["error"],
        json!({"kind":"invalid_state_file", "file":PROPOSALS, "line":second,
            "message":format!("review submission {proposal} is on lines {first} and {second} of \
                ideation/proposal_cards.jsonl with different contents"),
            "hint":"To repair, keep one of these lines and remove the others, then commit."}),
    );
}
