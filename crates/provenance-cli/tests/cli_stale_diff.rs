use assert_cmd::Command;
use predicates::prelude::*;
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
        "git {:?} failed: {}",
        args,
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
    git(repo, &["commit", "-m", message]);
    git(repo, &["rev-parse", "HEAD"])
}

fn init_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            dir.path().to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "requirements",
            "create",
            "--repo",
            dir.path().to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            "req_anchor",
            "--statement",
            "The anchor requirement holds",
        ])
        .assert()
        .success();
    dir
}

fn create_rule(repo: &Path, id: &str) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "rules",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            id,
            "--requirement-id",
            "req_anchor",
            "--statement",
            "Test rule",
        ])
        .assert()
        .success();
}

fn run(repo: &Path, args: &[&str]) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args(args)
        .args(["--repo", repo.to_str().unwrap(), "--scope", "default"])
        .assert()
        .success();
}

fn stale_json(repo: &Path, range: &[&str]) -> Value {
    let mut command = Command::cargo_bin("provenance").unwrap();
    command.args([
        "stale",
        "--repo",
        repo.to_str().unwrap(),
        "--format",
        "json",
    ]);
    command.args(range);
    let output = command.assert().success().get_output().stdout.clone();
    serde_json::from_slice(&output).unwrap()
}

fn site_states(report: &Value, state: &str) -> Vec<String> {
    report["sites"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|site| site["state"] == state)
        .map(|site| site["subject_id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn diff_inside_one_bound_function_touches_exactly_that_site() {
    let dir = init_repo();
    create_rule(dir.path(), "rule_guard");
    create_rule(dir.path(), "rule_other");
    write(
        dir.path(),
        "src/guards.rs",
        "#[rule(\"rule_guard\")]\nfn guard() { reject(); }\n\n#[rule(\"rule_other\")]\nfn other() { allow(); }\n",
    );
    let base = commit(dir.path(), "base");
    write(
        dir.path(),
        "src/guards.rs",
        "#[rule(\"rule_guard\")]\nfn guard() { reject_loudly(); }\n\n#[rule(\"rule_other\")]\nfn other() { allow(); }\n",
    );
    let head = commit(dir.path(), "change guard");

    let report = stale_json(dir.path(), &[&base, &head]);
    assert_eq!(site_states(&report, "touched"), ["rule_guard"]);
    assert_eq!(site_states(&report, "untouched"), ["rule_other"]);
    assert_eq!(report["summary"]["touched"], 1);
}

#[test]
fn diff_in_an_uncited_file_touches_no_evidence() {
    let dir = init_repo();
    create_rule(dir.path(), "rule_guard");
    write(
        dir.path(),
        "src/guard.rs",
        "#[rule(\"rule_guard\")]\nfn guard() { reject(); }\n",
    );
    let base = commit(dir.path(), "base");
    write(dir.path(), "notes.txt", "unrelated\n");
    commit(dir.path(), "notes");

    let report = stale_json(dir.path(), &["--since", &base]);
    assert!(site_states(&report, "touched").is_empty());
    assert_eq!(site_states(&report, "untouched"), ["rule_guard"]);
}

#[test]
fn relocated_unchanged_anchor_is_moved_not_touched() {
    let dir = init_repo();
    create_rule(dir.path(), "rule_guard");
    write(
        dir.path(),
        "src/guard.rs",
        "#[rule(\"rule_guard\")]\nfn guard() { reject(); }\n",
    );
    let base = commit(dir.path(), "base");
    write(
        dir.path(),
        "src/guard.rs",
        "// module note\n#[rule(\"rule_guard\")]\nfn guard() { reject(); }\n",
    );
    let head = commit(dir.path(), "move guard");

    let report = stale_json(dir.path(), &[&base, &head]);
    assert_eq!(site_states(&report, "moved"), ["rule_guard"]);
    assert!(site_states(&report, "touched").is_empty());
    assert_eq!(report["sites"][0]["original_line"], 1);
    assert_eq!(report["sites"][0]["line"], 2);
}

#[test]
fn cross_file_relocation_does_not_absorb_unrelated_code_into_the_site() {
    let dir = init_repo();
    create_rule(dir.path(), "rule_guard");
    write(
        dir.path(),
        "src/old.rs",
        "#[rule(\"rule_guard\")]\nfn guard() { reject(); }\n\nfn unrelated() { allow(); }\n",
    );
    let base = commit(dir.path(), "base");
    write(dir.path(), "src/old.rs", "fn unrelated() { allow(); }\n");
    write(
        dir.path(),
        "src/new.rs",
        "#[rule(\"rule_guard\")]\nfn guard() { reject(); }\n",
    );
    let head = commit(dir.path(), "move guard");

    let report = stale_json(dir.path(), &[&base, &head]);
    assert_eq!(site_states(&report, "moved"), ["rule_guard"]);
    assert!(site_states(&report, "touched").is_empty());
}

#[test]
fn strict_fails_for_touched_and_gone_but_plain_report_succeeds() {
    let touched = init_repo();
    create_rule(touched.path(), "rule_guard");
    write(
        touched.path(),
        "src/guard.rs",
        "#[rule(\"rule_guard\")]\nfn guard() { reject(); }\n",
    );
    let touched_base = commit(touched.path(), "base");
    write(
        touched.path(),
        "src/guard.rs",
        "#[rule(\"rule_guard\")]\nfn guard() { reject_loudly(); }\n",
    );
    let touched_head = commit(touched.path(), "touch");

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "stale",
            &touched_base,
            &touched_head,
            "--repo",
            touched.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("# Evidence Diff"))
        .stdout(predicate::str::contains("touched"));
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "stale",
            &touched_base,
            &touched_head,
            "--repo",
            touched.path().to_str().unwrap(),
            "--strict",
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("touched"));

    let gone = init_repo();
    create_rule(gone.path(), "rule_guard");
    write(
        gone.path(),
        "src/guard.rs",
        "#[rule(\"rule_guard\")]\nfn guard() { reject(); }\n",
    );
    let gone_base = commit(gone.path(), "base");
    write(gone.path(), "src/guard.rs", "fn guard() { reject(); }\n");
    let gone_head = commit(gone.path(), "remove binding");
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "stale",
            &gone_base,
            &gone_head,
            "--repo",
            gone.path().to_str().unwrap(),
            "--strict",
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"gone\": 1"));
}

#[test]
fn strict_succeeds_when_only_untouched_evidence_remains() {
    let dir = init_repo();
    create_rule(dir.path(), "rule_guard");
    write(
        dir.path(),
        "src/guard.rs",
        "#[rule(\"rule_guard\")]\nfn guard() { reject(); }\n",
    );
    let base = commit(dir.path(), "base");
    write(dir.path(), "notes.txt", "unrelated\n");
    let head = commit(dir.path(), "notes");

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "stale",
            &base,
            &head,
            "--repo",
            dir.path().to_str().unwrap(),
            "--strict",
        ])
        .assert()
        .success();
}

#[test]
fn referenced_source_code_path_participates_in_the_gate() {
    let dir = init_repo();
    run(
        dir.path(),
        &[
            "sources",
            "create",
            "--id",
            "source_policy",
            "--name",
            "Policy",
            "--reference",
            "owner decision; ./docs/policy.md:2",
        ],
    );
    run(
        dir.path(),
        &[
            "requirements",
            "create",
            "--id",
            "req_policy",
            "--statement",
            "Follow policy",
        ],
    );
    run(
        dir.path(),
        &[
            "requirements",
            "source-ref",
            "add",
            "--requirement-id",
            "req_policy",
            "--source-id",
            "source_policy",
        ],
    );
    write(dir.path(), "docs/policy.md", "# Policy\nOriginal text\n");
    let base = commit(dir.path(), "base");
    write(dir.path(), "docs/policy.md", "# Policy\nChanged text\n");
    let head = commit(dir.path(), "update policy");

    let report = stale_json(dir.path(), &[&base, &head]);
    assert_eq!(site_states(&report, "touched"), ["source_policy"]);
    assert_eq!(report["sites"][0]["kind"], "source_reference");
    assert_eq!(report["sites"][0]["line"], 2);
}

#[test]
fn verification_sites_are_reported_separately_from_rule_bindings() {
    let dir = init_repo();
    create_rule(dir.path(), "rule_guard");
    write(
        dir.path(),
        "src/guard.rs",
        "#[rule(\"rule_guard\")]\nfn guard() { reject(); }\n\n#[verifies(\"rule_guard\", examples)]\nfn rejects_examples() { assert_rejected(); }\n",
    );
    let base = commit(dir.path(), "base");
    write(
        dir.path(),
        "src/guard.rs",
        "#[rule(\"rule_guard\")]\nfn guard() { reject(); }\n\n#[verifies(\"rule_guard\", examples)]\nfn rejects_examples() { assert_all_rejected(); }\n",
    );
    let head = commit(dir.path(), "strengthen verification");

    let report = stale_json(dir.path(), &[&base, &head]);
    let touched = report["sites"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|site| site["state"] == "touched")
        .collect::<Vec<_>>();
    assert_eq!(touched.len(), 1);
    assert_eq!(touched[0]["kind"], "verification");
}

#[test]
fn comment_sites_use_implementation_and_verification_roles() {
    let dir = init_repo();
    create_rule(dir.path(), "rule_impl");
    create_rule(dir.path(), "rule_verified");
    write(
        dir.path(),
        "src/guards.rs",
        "// @provenance rule: rule_impl\nfn guard() { reject(); }\n\n\
         // @provenance rule: rule_verified\n\
         // @provenance verification: examples\nfn rejects_examples() { assert_rejected(); }\n",
    );
    let base = commit(dir.path(), "base");
    write(
        dir.path(),
        "src/guards.rs",
        "// @provenance rule: rule_impl\nfn guard() { reject_loudly(); }\n\n\
         // @provenance rule: rule_verified\n\
         // @provenance verification: examples\nfn rejects_examples() { assert_all_rejected(); }\n",
    );
    let head = commit(dir.path(), "change comment-bound sites");

    let report = stale_json(dir.path(), &[&base, &head]);
    let sites = report["sites"].as_array().unwrap();
    let implementation = sites
        .iter()
        .find(|site| site["subject_id"] == "rule_impl")
        .unwrap();
    let verification = sites
        .iter()
        .find(|site| site["subject_id"] == "rule_verified")
        .unwrap();

    assert_eq!(implementation["kind"], "rule_binding");
    assert_eq!(verification["kind"], "verification");
}
