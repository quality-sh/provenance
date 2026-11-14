//! Two branches each withdraw one review submission, and Git merges the
//! branches. The merged state holds two Withdrawal records of the
//! submission. The reads then show the review state that one withdrawal
//! gives, and the review writes of the scope continue.

use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

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

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn git_output(repo: &Path, args: &[&str]) -> std::process::Output {
    provenance_store::git_process::git()
        .args([
            "-c",
            "user.name=Reviewer",
            "-c",
            "user.email=reviewer@example.com",
        ])
        .args(["-c", "commit.gpgsign=false", "-c", "core.autocrlf=false"])
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap()
}

fn git(repo: &Path, args: &[&str]) -> String {
    let output = git_output(repo, args);
    assert!(output.status.success(), "git {args:?} failed: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

fn commit(repo: &Path, message: &str) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", message]);
}

/// Runs one command that must succeed and returns its JSON output.
fn read(repo: &Path, args: &[&str]) -> Value {
    let output = provenance()
        .args(args)
        .arg("--repo")
        .arg(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "provenance {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// Sends one write through the public API and returns its output.
fn api_write(repo: &Path, path: &str, body: &Value) -> std::process::Output {
    provenance()
        .args(["api", path, "--method", "post", "--input", "-", "--repo"])
        .arg(repo)
        .write_stdin(body.to_string())
        .output()
        .unwrap()
}

fn written(repo: &Path, path: &str, body: &Value) -> Value {
    let output = api_write(repo, path, body);
    assert!(
        output.status.success(),
        "{path}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn create(repo: &Path, id: &str) {
    written(
        repo,
        "requirements",
        &json!({"actor":"agent", "id":id, "statement":"The system keeps one copy of each record.",
            "status":"active", "depends_on":[], "supersedes":[]}),
    );
}

/// The pending review submission of one Requirement.
fn pending(repo: &Path, record: &str) -> String {
    let path = format!("requirements/{record}");
    let decision = &read(repo, &["api", &path])["data"]["decision"];
    decision["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn withdraw(repo: &Path, record: &str, submission: &str, actor: &str, reason: &str) {
    written(
        repo,
        &format!("requirements/{record}/submissions/{submission}/withdraw"),
        &json!({"actor":actor, "declared_by":null, "reason":reason}),
    );
}

/// A new repository with `req_a` and `req_b` committed on `main`.
fn repository(merge: Merge) -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    git(repo, &["init", "-q", "--initial-branch", "main"]);
    if merge == Merge::Driver {
        let program = assert_cmd::cargo::cargo_bin!("provenance");
        // Git runs the driver through sh, which removes backslashes.
        let program = program.to_str().unwrap().replace('\\', "/");
        let driver = format!("'{program}' merge-jsonl %O %A %B --output %A --path %P");
        git(repo, &["config", "merge.provenance-jsonl.driver", &driver]);
        let attributes = ".provenance/state/**/*.jsonl merge=provenance-jsonl\n";
        std::fs::write(repo.join(".gitattributes"), attributes).unwrap();
    }
    git(repo, &["config", "user.email", "reviewer@example.com"]);
    provenance()
        .args(["init", "--scope", "default", "--path-prefix", ".", "--path"])
        .arg(repo)
        .assert()
        .success();
    create(repo, "req_a");
    create(repo, "req_b");
    commit(repo, "Add the Requirements");
    directory
}

/// Merges `side` into `main` in the way `merge` names.
fn merge_side(repo: &Path, merge: Merge) {
    let merged = git_output(repo, &["merge", "--no-edit", "side"]);
    match merge {
        Merge::Driver => assert!(merged.status.success(), "{merged:?}"),
        Merge::KeepBoth => {
            assert!(!merged.status.success(), "{merged:?}");
            let conflicts = git(repo, &["diff", "--name-only", "--diff-filter=U"]);
            assert_eq!(conflicts.trim(), WITHDRAWALS);
            let ours = git(repo, &["show", &format!(":2:{WITHDRAWALS}")]);
            let theirs = git(repo, &["show", &format!(":3:{WITHDRAWALS}")]);
            std::fs::write(repo.join(WITHDRAWALS), ours + &theirs).unwrap();
            commit(repo, "Keep both withdrawals");
        }
    }
}

fn check_passes(repo: &Path) {
    provenance()
        .arg("check")
        .arg("--repo")
        .arg(repo)
        .assert()
        .success();
}

/// The review reads of the scope that show the submission or its record.
fn review_reads(repo: &Path) -> Vec<Value> {
    let view = read(repo, REVIEW_VIEW);
    vec![
        read(repo, &["api", "requirements/req_a"])["data"]["decision"].clone(),
        view["review"]["decision"].clone(),
        read(repo, &["api", "requirements/req_a/document"])["data"].clone(),
        read(repo, &["api", "requirements/req_b"])["data"]["decision"].clone(),
    ]
}

fn two_withdrawals_read_as_one(merge: Merge) {
    let directory = repository(merge);
    let repo = directory.path();
    let submission = pending(repo, "req_a");
    git(repo, &["checkout", "-q", "-b", "side"]);
    withdraw(repo, "req_a", &submission, "agent", "Side branch.");
    commit(repo, "Withdraw on side");
    git(repo, &["checkout", "-q", "main"]);
    withdraw(repo, "req_a", &submission, "editor", "Main branch.");
    commit(repo, "Withdraw on main");
    let one_withdrawal = review_reads(repo);

    merge_side(repo, merge);

    let rows = std::fs::read_to_string(repo.join(WITHDRAWALS)).unwrap();
    assert_eq!(rows.matches(submission.as_str()).count(), 2, "{rows}");
    assert_eq!(review_reads(repo), one_withdrawal);
    assert_eq!(one_withdrawal[0]["withdrawn"], json!([submission]));
    assert_eq!(one_withdrawal[0]["pending"], Value::Null);
    read(repo, &["api", "requirements"]);
    check_passes(repo);

    let again = api_write(
        repo,
        &format!("requirements/req_a/submissions/{submission}/withdraw"),
        &json!({"actor":"agent", "declared_by":null, "reason":"Withdrawn again."}),
    );
    assert!(!again.status.success());
    let stderr = String::from_utf8_lossy(&again.stderr);
    let envelope = stderr.trim().strip_prefix("Error: ").unwrap();
    let refusal: Value = serde_json::from_str(envelope).unwrap();
    assert_eq!(refusal["error"]["kind"], "review_submission_conflict");
    assert_eq!(refusal["error"]["current_submission"], Value::Null);
    let resubmitted = written(
        repo,
        "requirements/req_a/submit",
        &json!({"actor":"agent", "declared_by":null, "title":"Review req_a",
            "summary":"Review the statement again.", "confidence":null, "source_ids":[],
            "evidence_references":[], "builds_on":[], "expected_revision":null, "revises":null}),
    );
    assert_eq!(resubmitted["data"]["fact"], "submitted");
    let other = pending(repo, "req_b");
    withdraw(repo, "req_b", &other, "agent", "After the merge.");
    check_passes(repo);
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
