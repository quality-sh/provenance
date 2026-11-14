//! Two branches each withdraw one review submission, and Git merges the
//! branches. The merged state holds two Withdrawal records of the
//! submission. The reads then show the review state that one withdrawal
//! gives, and the review writes of the scope continue.

use super::review_cycle::{init_with_reviewer, write};
use super::review_history::{commit, git, git_output};
use super::*;

const WITHDRAWALS: &str = ".provenance/state/scopes/default/ideation/withdrawals.jsonl";
const REVIEW_VIEW: &[&str] = &["req_a", "get", "--view", "review", "--format", "json"];

/// How Git merges the two branches.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Merge {
    /// The repository sets up the JSONL merge driver, and the merge is clean.
    Driver,
    /// The repository has no merge driver. The two additions of the
    /// Withdrawal file conflict, and the person keeps both sides.
    KeepBoth,
}

/// Runs one command on `repo` that must succeed and returns its JSON output.
fn read(repo: &str, arguments: &[&str]) -> Value {
    let mut arguments = arguments.to_vec();
    arguments.extend(["--repo", repo]);
    json(&arguments)
}

fn create(repo: &str, id: &str) {
    write(
        repo,
        "post",
        "requirements",
        &json!({"actor":"agent", "id":id, "statement":"The system keeps one copy of each record.",
            "status":"active", "depends_on":[], "supersedes":[]}),
        None,
    );
}

/// The pending review submission of one Requirement.
fn pending(repo: &str, record: &str) -> String {
    let path = format!("requirements/{record}");
    let decision = &read(repo, &["api", &path])["data"]["decision"];
    decision["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn withdraw(repo: &str, record: &str, submission: &str, actor: &str, reason: &str) {
    let path = format!("requirements/{record}/submissions/{submission}/withdraw");
    let body = json!({"actor":actor, "declared_by":null, "reason":reason});
    write(repo, "post", &path, &body, None);
}

/// A new repository with `req_a` and `req_b` committed.
fn repository(merge: Merge) -> (tempfile::TempDir, String) {
    let (directory, repo) = init_with_reviewer();
    if merge == Merge::Driver {
        let program = assert_cmd::cargo::cargo_bin!("provenance");
        // Git runs the driver through sh, which removes backslashes.
        let program = program.to_str().unwrap().replace('\\', "/");
        let driver = format!("'{program}' merge-jsonl %O %A %B --output %A --path %P");
        git(&repo, &["config", "merge.provenance-jsonl.driver", &driver]);
        let attributes = ".provenance/state/**/*.jsonl merge=provenance-jsonl\n";
        directory_file(&repo, ".gitattributes", attributes);
        git(&repo, &["add", ".gitattributes"]);
    }
    create(&repo, "req_a");
    create(&repo, "req_b");
    commit(&repo);
    (directory, repo)
}

/// Merges `side` into the first branch in the way `merge` names.
fn merge_side(repo: &str, merge: Merge) {
    let merged = git_output(repo, &["merge", "--no-edit", "side"]);
    match merge {
        Merge::Driver => assert!(merged.status.success(), "{merged:?}"),
        Merge::KeepBoth => {
            assert!(!merged.status.success(), "{merged:?}");
            let conflicts = git(repo, &["diff", "--name-only", "--diff-filter=U"]);
            assert_eq!(conflicts, WITHDRAWALS);
            let ours = git(repo, &["show", &format!(":2:{WITHDRAWALS}")]);
            let theirs = git(repo, &["show", &format!(":3:{WITHDRAWALS}")]);
            directory_file(repo, WITHDRAWALS, &format!("{ours}\n{theirs}\n"));
            commit(repo);
        }
    }
}

/// `provenance check` passes on `repo`.
fn check_passes(repo: &str) {
    success(&["check", "--repo", repo]);
}

/// The review reads of the scope that show the submission or its record.
fn review_reads(repo: &str) -> Vec<Value> {
    let view = read(repo, REVIEW_VIEW);
    vec![
        read(repo, &["api", "requirements/req_a"])["data"]["decision"].clone(),
        view["review"]["decision"].clone(),
        read(repo, &["api", "requirements/req_a/document"])["data"].clone(),
        read(repo, &["api", "requirements/req_b"])["data"]["decision"].clone(),
    ]
}

fn two_withdrawals_read_as_one(merge: Merge) {
    let (_directory, repo) = repository(merge);
    let submission = pending(&repo, "req_a");
    let first = git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    git(&repo, &["checkout", "-q", "-b", "side"]);
    withdraw(&repo, "req_a", &submission, "agent", "Side branch.");
    commit(&repo);
    git(&repo, &["checkout", "-q", &first]);
    withdraw(&repo, "req_a", &submission, "editor", "First branch.");
    commit(&repo);
    let one_withdrawal = review_reads(&repo);

    merge_side(&repo, merge);

    let file = std::path::Path::new(&repo).join(WITHDRAWALS);
    let rows = std::fs::read_to_string(file).unwrap();
    let mut rows = rows
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row["actor"].as_str().unwrap().to_owned());
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_ne!(rows[0]["id"], rows[1]["id"], "{rows:?}");
    assert_eq!(rows[0]["proposal_id"], json!(submission));
    assert_eq!(rows[1]["proposal_id"], json!(submission));
    assert_eq!(
        (&rows[0]["actor"], &rows[1]["actor"]),
        (&json!("agent"), &json!("editor"))
    );
    assert_eq!(review_reads(&repo), one_withdrawal);
    assert_eq!(one_withdrawal[0]["withdrawn"], json!([submission]));
    assert_eq!(one_withdrawal[0]["pending"], Value::Null);
    read(&repo, &["api", "requirements"]);
    check_passes(&repo);

    let path = format!("requirements/req_a/submissions/{submission}/withdraw");
    let body = json!({"actor":"agent", "declared_by":null, "reason":"Withdrawn again."});
    let input = directory_file(&repo, "withdrawal.json", &body.to_string());
    let mut arguments: Vec<&str> = vec!["api", &path, "--method", "post"];
    arguments.extend(["--input", &input, "--repo", &repo]);
    let refused = refusal(&arguments);
    assert_eq!(refused["error"]["kind"], "review_submission_conflict");
    assert_eq!(refused["error"]["current_submission"], Value::Null);
    let resubmitted = write(
        &repo,
        "post",
        "requirements/req_a/submit",
        &json!({"actor":"agent", "declared_by":null, "title":"Review req_a",
            "summary":"Review the statement again.", "confidence":null, "source_ids":[],
            "evidence_references":[], "builds_on":[], "expected_revision":null, "revises":null}),
        None,
    );
    assert_eq!(resubmitted["data"]["fact"], "submitted");
    let other = pending(&repo, "req_b");
    withdraw(&repo, "req_b", &other, "agent", "After the merge.");
    check_passes(&repo);
}

/// Flow: two branches withdraw one submission, the merge driver merges them
/// with no conflict, and each review read and write of the scope works.
#[test]
#[verifies("rule_repeated_withdrawals_read_as_one", examples)]
fn two_withdrawals_merged_by_the_driver_read_as_one() {
    two_withdrawals_read_as_one(Merge::Driver);
}

/// Flow: two branches withdraw one submission, a merge with no driver
/// conflicts, the person keeps both sides, and each review read and write of
/// the scope works.
#[test]
#[verifies("rule_repeated_withdrawals_read_as_one", examples)]
fn two_withdrawals_kept_by_a_person_read_as_one() {
    two_withdrawals_read_as_one(Merge::KeepBoth);
}
