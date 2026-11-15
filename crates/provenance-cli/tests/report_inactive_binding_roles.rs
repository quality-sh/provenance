//! The branch report names each current binding to a deprecated or archived
//! Rule by the role of the binding: a test is a verification and production
//! code is an implementation, in the comment form and in the attribute form.

use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::Value;
use std::path::Path;

use crate::support::record_review::archive_rule;

fn git(repo: &Path, args: &[&str]) -> String {
    let output = provenance_store::git_process::git()
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

fn create(repo: &Path, args: &[&str]) {
    provenance(repo)
        .args(args)
        .args(["--repo", repo.to_str().unwrap(), "--scope", "default"])
        .assert()
        .success();
}

/// A repository with one deprecated Rule and one archived Rule.
fn repo_with_inactive_rules() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    git(repo, &["init", "-q"]);
    git(repo, &["config", "user.email", "test@example.com"]);
    git(repo, &["config", "user.name", "Test"]);
    write(repo, "README.md", "Payroll\n");
    provenance(repo)
        .args(["init", "--path", repo.to_str().unwrap()])
        .args(["--scope", "default", "--path-prefix", "."])
        .args(["--disposition-actor-id", "reviewer"])
        .assert()
        .success();
    create(
        repo,
        &[
            "req_anchor",
            "create",
            "--type",
            "requirement",
            "--statement",
            "The anchor requirement holds",
        ],
    );
    for (id, status) in [
        ("rule_deprecated", "deprecated"),
        ("rule_archived", "draft"),
    ] {
        create(
            repo,
            &[
                id,
                "create",
                "--type",
                "rule",
                "--requirement-id",
                "req_anchor",
                "--statement",
                "The anchor rule holds",
                "--status",
                status,
            ],
        );
    }
    archive_rule(repo.to_str().unwrap(), "rule_archived", &"a".repeat(40));
    directory
}

/// A test in each language that the scanner reads, by file extension.
const TESTS: [(&str, &str); 6] = [
    ("rs", "#[test]\nfn keeps_each_record() {}\n"),
    ("py", "def test_keeps_each_record():\n    pass\n"),
    ("go", "func TestKeepsEachRecord(t *testing.T) {}\n"),
    ("java", "@Test\nvoid keepsEachRecord() {}\n"),
    ("js", "test(\"keeps\", function keepsEachRecord() {});\n"),
    ("ts", "test(\"keeps\", function keepsEachRecord() {});\n"),
];

/// One test that cites `rule` with the comment form, in each language that
/// the scanner reads.
fn comment_tests(repo: &Path, rule: &str) {
    for (extension, test) in TESTS {
        let comment = if extension == "py" { "#" } else { "//" };
        let marker = format!("{comment} @provenance verification: examples\n");
        let source = format!("{marker}{comment} @provenance rule: {rule}\n{test}");
        write(repo, &format!("{rule}/comment_test.{extension}"), &source);
    }
}

/// The sites of the inactive-Rule finding for `rule`, as path, role, and method.
fn sites(envelope: &Value, rule: &str) -> Vec<(String, String, Option<String>)> {
    let finding = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| {
            finding["code"] == "inactive_rule_current_binding" && finding["subject"]["id"] == rule
        })
        .unwrap_or_else(|| panic!("no inactive-Rule finding for {rule}: {envelope}"));
    let mut sites = finding["sites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|site| {
            (
                site["path"].as_str().unwrap().to_owned(),
                site["role"].as_str().unwrap().to_owned(),
                site["method"].as_str().map(str::to_owned),
            )
        })
        .collect::<Vec<_>>();
    sites.sort();
    sites
}

fn expected(rule: &str) -> Vec<(String, String, Option<String>)> {
    let verification = |path: &str| {
        (
            format!("{rule}/{path}"),
            "verification".to_owned(),
            Some("examples".to_owned()),
        )
    };
    let implementation = |path: &str| {
        (format!("{rule}/{path}"), "implementation".to_owned(), None)
    };
    let mut sites = vec![
        verification("attribute_test.rs"),
        implementation("comment_implementation.rs"),
    ];
    sites.extend(
        ["go", "java", "js", "py", "rs", "ts"]
            .map(|extension| verification(&format!("comment_test.{extension}"))),
    );
    sites.sort();
    sites
}

#[test]
#[verifies("rule_report_inactive_test_site_is_a_verification", examples)]
fn a_test_comment_on_an_inactive_rule_is_a_verification_in_each_language() {
    let directory = repo_with_inactive_rules();
    let repo = directory.path();
    let base = commit(repo, "inactive Rules");
    for rule in ["rule_deprecated", "rule_archived"] {
        comment_tests(repo, rule);
        let attribute = format!("#[verifies(\"{rule}\", examples)]\n");
        let attribute_test = format!("#[test]\n{attribute}fn keeps_each_record() {{}}\n");
        write(repo, &format!("{rule}/attribute_test.rs"), &attribute_test);
        let implementation = format!("// @provenance rule: {rule}\npub fn keep_record() {{}}\n");
        write(repo, &format!("{rule}/comment_implementation.rs"), &implementation);
    }
    let head = commit(repo, "bindings to inactive Rules");

    let output = provenance(repo)
        .args(["report", "build", "--repo"])
        .arg(repo)
        .args([
            "--base",
            &base,
            "--head",
            &head,
            "--repository",
            "example/payroll",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();

    for rule in ["rule_deprecated", "rule_archived"] {
        assert_eq!(sites(&envelope, rule), expected(rule), "{rule}");
    }
}
