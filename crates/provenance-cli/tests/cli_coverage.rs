use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::path::Path;

fn strict_validating_scan(repo: &std::path::Path, source_dir: &std::path::Path) -> Command {
    let mut command = Command::cargo_bin("provenance").unwrap();
    command.args([
        "coverage",
        "scan",
        "--repo",
        repo.to_str().unwrap(),
        "--path",
        source_dir.to_str().unwrap(),
        "--scope",
        "default",
        "--validate-rules",
        "--strict",
        "--format",
        "json",
    ]);
    command
}

#[test]
fn coverage_scan_reports_unknown_rule_warnings() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    let source_dir = repo.join("src");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(
        source_dir.join("payroll.rs"),
        "// @provenance rule: UNKNOWN-RULE\nfn pays_overtime() {}\n",
    )
    .unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
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

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "coverage",
            "scan",
            "--repo",
            repo.to_str().unwrap(),
            "--path",
            source_dir.to_str().unwrap(),
            "--scope",
            "default",
            "--validate-rules",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("UNKNOWN-RULE"))
        .stdout(predicate::str::contains("total_annotations"));
}

#[test]
fn coverage_scan_writes_markdown_output_file() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    let source_dir = repo.join("src");
    let output = repo.join("coverage.md");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(
        source_dir.join("payroll.py"),
        "# @provenance rule: SCHADS-PAY-001\ndef pays_overtime():\n    pass\n",
    )
    .unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
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

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "coverage",
            "scan",
            "--repo",
            repo.to_str().unwrap(),
            "--path",
            source_dir.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "markdown",
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let markdown = std::fs::read_to_string(output).unwrap();
    assert!(markdown.contains("# Coverage Scan"));
    assert!(markdown.contains("SCHADS-PAY-001"));
}

/// A change author reading the report wants to know who leans on an
/// implementation from another module, because that is whose tests a change
/// to the implementation breaks.
#[test]
fn coverage_markdown_marks_verification_sites_outside_the_implementation_module() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    let source_dir = repo.join("src");
    let output = repo.join("coverage.md");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(
        source_dir.join("payroll.rs"),
        "#[rule(\"rule_pays_overtime\")]\npub fn pays_overtime() {}\n\n#[verifies(\"rule_pays_overtime\", exhaustion)]\nfn covers_every_threshold() {}\n",
    )
    .unwrap();
    std::fs::write(
        source_dir.join("billing.rs"),
        "#[verifies(\"rule_pays_overtime\", examples)]\nfn bills_overtime_at_the_right_rate() {}\n",
    )
    .unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
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

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "coverage",
            "scan",
            "--repo",
            repo.to_str().unwrap(),
            "--path",
            source_dir.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "markdown",
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let markdown = std::fs::read_to_string(output).unwrap();
    assert!(
        markdown.contains("verified by examples at `")
            && markdown.contains(
                "billing.rs`:1 (bills_overtime_at_the_right_rate) (new) (outside implementation module)"
            ),
        "billing site not marked as outside the implementation module:\n{markdown}"
    );
    assert!(
        !markdown.contains("covers_every_threshold) (outside implementation module)"),
        "site beside the rule wrongly marked:\n{markdown}"
    );
}

#[test]
fn strict_scan_reports_unverified_and_unimplemented_as_independent_findings() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    let source_dir = repo.join("src");
    std::fs::create_dir_all(&source_dir).unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
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

    Command::cargo_bin("provenance")
        .unwrap()
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
            "rule_pays_overtime",
            "--requirement-id",
            "req_anchor",
            "--statement",
            "Pay overtime after the threshold",
            "--severity",
            "high",
        ])
        .assert()
        .success();

    std::fs::write(source_dir.join("payroll.rs"), "fn pays_overtime() {}\n").unwrap();

    // Unverified: --strict fails, report still printed.
    strict_validating_scan(repo, repo)
        .assert()
        .failure()
        .stdout(predicate::str::contains("has no verification"));

    // Verification does not imply implementation. The old dangling-verifies
    // warning is gone because the canonical Rule exists.
    std::fs::write(
        source_dir.join("payroll.rs"),
        "#[verifies(\"rule_pays_overtime\", examples)]\nfn verifies_pays_overtime() {}\n",
    )
    .unwrap();

    strict_validating_scan(repo, repo)
        .assert()
        .failure()
        .stdout(predicate::str::contains("has no implementation"))
        .stdout(predicate::str::contains("has no #[rule]").not())
        .stdout(predicate::str::contains("has no verification").not());

    std::fs::write(
        source_dir.join("payroll.rs"),
        "#[rule(\"rule_pays_overtime\")]\nfn pays_overtime() {}\n\n#[verifies(\"rule_pays_overtime\", examples)]\nfn verifies_pays_overtime() {}\n",
    )
    .unwrap();

    strict_validating_scan(repo, repo).assert().success();
}

#[test]
fn partial_scan_does_not_claim_scope_wide_binding_absence() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    let selected = repo.join("selected");
    std::fs::create_dir_all(&selected).unwrap();
    std::fs::write(selected.join("unrelated.rs"), "fn unrelated() {}\n").unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
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
    Command::cargo_bin("provenance")
        .unwrap()
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
            "rule_outside_selected_path",
            "--requirement-id",
            "req_anchor",
            "--statement",
            "A Rule outside the selected scan territory",
        ])
        .assert()
        .success();

    strict_validating_scan(repo, &selected)
        .assert()
        .success()
        .stdout(predicate::str::contains("has no implementation").not())
        .stdout(predicate::str::contains("has no verification").not());
}

#[test]
fn coverage_scan_warns_for_deprecated_marker_but_not_active_marker() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    let source_dir = repo.join("src");
    std::fs::create_dir_all(&source_dir).unwrap();
    init_repo(repo);
    create_rule(repo, "rule_deprecated", "deprecated");
    create_rule(repo, "rule_archived", "archived");
    create_rule(repo, "rule_active", "active");
    std::fs::write(
        source_dir.join("payroll.rs"),
        "// @provenance rule: rule_deprecated\nfn old_payroll() {}\n\
         // @provenance rule: rule_archived\nfn archived_payroll() {}\n\
         // @provenance rule: rule_active\n// @provenance verification: examples\nfn current_payroll() {}\n\
         // @provenance rule: rule_missing\nfn missing_rule() {}\n",
    )
    .unwrap();

    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "coverage",
            "scan",
            "--repo",
            repo.to_str().unwrap(),
            "--path",
            source_dir.to_str().unwrap(),
            "--scope",
            "default",
            "--validate-rules",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let warnings = report["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 3, "unexpected warnings: {warnings:#?}");
    assert!(warnings.iter().any(|warning| {
        warning["rule_id"] == "rule_deprecated"
            && warning["message"]
                .as_str()
                .is_some_and(|message| message.contains("deprecated"))
    }));
    assert!(warnings.iter().any(|warning| {
        warning["rule_id"] == "rule_archived"
            && warning["message"]
                .as_str()
                .is_some_and(|message| message.contains("archived"))
    }));
    assert!(warnings.iter().any(|warning| {
        warning["rule_id"] == "rule_missing"
            && warning["message"] == "unknown rule id `rule_missing`"
    }));
    assert!(!warnings
        .iter()
        .any(|warning| warning["rule_id"] == "rule_active"));
}

#[test]
fn coverage_scan_strict_exits_non_zero_for_deprecated_marker() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    let source_dir = repo.join("src");
    std::fs::create_dir_all(&source_dir).unwrap();
    init_repo(repo);
    create_rule(repo, "rule_deprecated", "deprecated");
    std::fs::write(
        source_dir.join("payroll.rs"),
        "// @provenance rule: rule_deprecated\nfn old_payroll() {}\n",
    )
    .unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "coverage",
            "scan",
            "--repo",
            repo.to_str().unwrap(),
            "--path",
            source_dir.to_str().unwrap(),
            "--scope",
            "default",
            "--validate-rules",
            "--strict",
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("rule_deprecated"))
        .stdout(predicate::str::contains("deprecated"));
}

fn init_repo(repo: &Path) {
    Command::cargo_bin("provenance")
        .unwrap()
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
    Command::cargo_bin("provenance")
        .unwrap()
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
}

fn create_rule(repo: &Path, id: &str, status: &str) {
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
            "Payroll follows the current policy",
            "--status",
            status,
            "--severity",
            "high",
        ])
        .assert()
        .success();
}
