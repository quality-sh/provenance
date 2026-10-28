//! Source-to-commit integrity tests for report building. The working copy
//! selects the scan surface, and requested-head blobs supply source facts.

use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::Value;
use std::path::Path;
use std::process::Command as ProcessCommand;

fn git(repo: &Path, args: &[&str]) -> String {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn write(repo: &Path, relative: &str, contents: &str) {
    let path = repo.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn commit(repo: &Path, message: &str) -> String {
    git(repo, &["add", "."]);
    git(repo, &["commit", "-q", "-m", message]);
    git(repo, &["rev-parse", "HEAD"])
}

fn provenance(repo: &Path) -> Command {
    let mut command = Command::cargo_bin("provenance").unwrap();
    command.current_dir(repo);
    command
}

fn repo_with_rule(status: &str) -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    git(repo, &["init", "-q"]);
    git(repo, &["config", "user.email", "test@example.com"]);
    git(repo, &["config", "user.name", "Test"]);
    write(repo, "src/lib.rs", "pub fn anchor() {}\n");
    provenance(repo)
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    provenance(repo)
        .args([
            "requirements",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            "req_anchor",
            "--statement",
            "The anchor requirement holds",
        ])
        .assert()
        .success();
    provenance(repo)
        .args([
            "rules",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            "rule_anchor",
            "--requirement-id",
            "req_anchor",
            "--statement",
            "The anchor rule holds",
            "--status",
            status,
        ])
        .assert()
        .success();
    directory
}

fn report_build(
    current_dir: &Path,
    repo: &Path,
    base: &str,
    head: &str,
    path: Option<&Path>,
) -> std::process::Output {
    let mut command = provenance(current_dir);
    command.args(["report", "build", "--repo"]).arg(repo).args([
        "--base",
        base,
        "--head",
        head,
        "--repository",
        "quality-sh/provenance",
    ]);
    if let Some(path) = path {
        command.arg("--path").arg(path);
    }
    command.output().unwrap()
}

fn envelope_from(output: &std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "report build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn build_envelope(repo: &Path, base: &str, head: &str, path: Option<&Path>) -> Value {
    envelope_from(&report_build(repo, repo, base, head, path))
}

fn finding<'a>(envelope: &'a Value, code: &str) -> Option<&'a Value> {
    envelope["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|finding| finding["code"] == code && finding["subject"]["id"] == "rule_anchor")
}

const fn source_binding(verification: bool) -> &'static str {
    if verification {
        "#[test]\n#[verifies(\"rule_anchor\", examples)]\nfn evidence() {}\n"
    } else {
        "#[rule(\"rule_anchor\")]\npub fn implementation() {}\n"
    }
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn historical_head_does_not_receive_a_later_checkout_site() {
    let directory = repo_with_rule("deprecated");
    let repo = directory.path();
    let requested_head = commit(repo, "requested head without a binding");
    write(repo, "src/lib.rs", source_binding(false));
    let checkout_head = commit(repo, "later checkout with a binding");
    assert_ne!(requested_head, checkout_head);

    let envelope = build_envelope(repo, &requested_head, &requested_head, None);

    assert_eq!(envelope["scan"]["completeness"], "incomplete");
    assert!(
        finding(&envelope, "inactive_rule_current_binding").is_none(),
        "a later checkout binding must not be attributed to the requested head"
    );
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn dirty_source_does_not_receive_a_head_site() {
    let directory = repo_with_rule("deprecated");
    let repo = directory.path();
    let head = commit(repo, "head without a binding");
    write(repo, "src/lib.rs", source_binding(false));

    let envelope = build_envelope(repo, &head, &head, None);

    assert_eq!(envelope["scan"]["completeness"], "incomplete");
    assert!(
        finding(&envelope, "inactive_rule_current_binding").is_none(),
        "dirty source bytes must not be attributed to the head"
    );
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn ignored_source_cannot_hide_a_known_absence() {
    let directory = repo_with_rule("active");
    let repo = directory.path();
    write(repo, ".gitignore", "ignored.rs\n");
    let head = commit(repo, "head without a verification");
    write(repo, "ignored.rs", source_binding(true));
    assert!(
        git(repo, &["status", "--porcelain", "--untracked-files=all"]).is_empty(),
        "the ignored fixture must leave Git status clean"
    );

    let envelope = build_envelope(repo, &head, &head, None);

    assert_eq!(envelope["scan"]["completeness"], "complete");
    let absence = finding(&envelope, "active_rule_missing_verification")
        .expect("ignored uncommitted bytes cannot suppress a head absence");
    assert_eq!(absence["binding_presence"], "absent");
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn partial_scan_keeps_a_valid_requested_head_site() {
    let directory = repo_with_rule("deprecated");
    let repo = directory.path();
    write(repo, "src/lib.rs", source_binding(false));
    let head = commit(repo, "head with a committed binding");

    let envelope = build_envelope(repo, &head, &head, Some(&repo.join("src")));

    assert_eq!(envelope["scan"]["completeness"], "incomplete");
    let current = finding(&envelope, "inactive_rule_current_binding")
        .expect("a committed binding remains valid in a partial scan");
    assert_eq!(current["binding_presence"], "present");
    assert_eq!(current["sites"][0]["commit"], "head");
    assert_eq!(current["sites"][0]["path"], "src/lib.rs");
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn partial_scan_without_source_files_reads_no_head_site() {
    let directory = repo_with_rule("deprecated");
    let repo = directory.path();
    write(repo, "src/lib.rs", source_binding(false));
    write(repo, "notes/plan.txt", "No source code here.\n");
    let head = commit(repo, "head with a binding outside the scan path");

    let envelope = build_envelope(repo, &head, &head, Some(&repo.join("notes")));

    assert_eq!(envelope["scan"]["completeness"], "incomplete");
    assert!(
        finding(&envelope, "inactive_rule_current_binding").is_none(),
        "a scan path with no source files must not read sites from the whole head tree"
    );
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn committed_typed_binding_survives_an_unpinned_source_scan() {
    let directory = repo_with_rule("deprecated");
    let repo = directory.path();
    write(
        repo,
        ".provenance/state/scopes/default/implementations/binding.jsonl",
        concat!(
            "{\"schema_version\":2,\"scope_id\":\"default\",",
            "\"id\":\"impl_anchor\",\"rule_id\":\"rule_anchor\",",
            "\"declared_by\":\"spec://test/anchor\",",
            "\"file\":\"src/lib.rs\",\"symbol\":\"anchor\"}\n"
        ),
    );
    let head = commit(repo, "head with a typed binding");
    write(repo, "src/lib.rs", "pub fn dirty() {}\n");

    let envelope = build_envelope(repo, &head, &head, None);

    let current = finding(&envelope, "inactive_rule_current_binding")
        .expect("a typed binding comes from the requested graph commit");
    assert_eq!(current["binding_presence"], "present");
    assert!(
        current["sites"].as_array().is_none_or(Vec::is_empty),
        "a typed binding has no fabricated source coordinate"
    );
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn clean_source_conversion_keeps_committed_verification() {
    let directory = repo_with_rule("active");
    let repo = directory.path();
    git(repo, &["config", "core.autocrlf", "true"]);
    write(
        repo,
        "src/lib.rs",
        "#[test]\n#[verifies(\"rule_anchor\", examples)]\nfn evidence() {}\n",
    );
    let head = commit(repo, "head with a verification");
    std::fs::remove_file(repo.join("src/lib.rs")).unwrap();
    git(repo, &["checkout", "--", "src/lib.rs"]);
    assert!(
        std::fs::read(repo.join("src/lib.rs"))
            .unwrap()
            .windows(2)
            .any(|bytes| bytes == b"\r\n"),
        "Git must convert the checked-out source to CRLF"
    );
    assert!(
        git(repo, &["status", "--porcelain", "--untracked-files=all"]).is_empty(),
        "Git must consider the converted source clean"
    );

    let envelope = build_envelope(repo, &head, &head, None);

    assert_eq!(envelope["scan"]["completeness"], "complete");
    assert!(
        finding(&envelope, "active_rule_missing_verification").is_none(),
        "a clean Git conversion must retain the committed verification"
    );
}

fn repo_with_committed_verification() -> (tempfile::TempDir, String) {
    let directory = repo_with_rule("active");
    let repo = directory.path();
    write(repo, "src/lib.rs", source_binding(true));
    let head = commit(repo, "head with a verification");
    (directory, head)
}

fn assert_verification_is_scanned(envelope: &Value) {
    assert_eq!(envelope["scan"]["completeness"], "complete");
    assert_eq!(envelope["scan"]["files_scanned"], 1);
    assert!(
        finding(envelope, "active_rule_missing_verification").is_none(),
        "the committed verification must be scanned"
    );
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn scan_path_in_a_different_form_from_repo_keeps_scanned_files() {
    let (directory, head) = repo_with_committed_verification();
    let repo = directory.path();

    let output = report_build(repo, Path::new("."), &head, &head, Some(repo));

    assert_verification_is_scanned(&envelope_from(&output));
}

#[cfg(unix)]
#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn symlinked_repo_path_keeps_scanned_files() {
    let (directory, head) = repo_with_committed_verification();
    let repo = directory.path();
    let links = tempfile::tempdir().unwrap();
    let link = links.path().join("repo");
    std::os::unix::fs::symlink(repo, &link).unwrap();

    let output = report_build(repo, &link, &head, &head, Some(repo));

    assert_verification_is_scanned(&envelope_from(&output));
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn scan_path_outside_the_repository_is_refused() {
    let (directory, head) = repo_with_committed_verification();
    let repo = directory.path();
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), "src/lib.rs", source_binding(true));

    let output = report_build(repo, repo, &head, &head, Some(outside.path()));

    assert!(
        !output.status.success(),
        "a scan path outside the repository must be refused, not dropped"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("outside the repository"),
        "the refusal must name the cause: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn files_scanned_counts_files_at_the_requested_head() {
    let (directory, head) = repo_with_committed_verification();
    let repo = directory.path();
    write(repo, "src/untracked.rs", "pub fn untracked() {}\n");

    let envelope = build_envelope(repo, &head, &head, None);

    assert_eq!(envelope["scan"]["completeness"], "incomplete");
    assert_eq!(envelope["scan"]["files_scanned"], 1);
}
