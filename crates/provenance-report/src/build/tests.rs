use super::{build_envelope, BuildInput};
use crate::envelope::{Comparison, SiteRole};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{Manifest, RepoPathPrefix, ScopeId, SUPPORTED_SCHEMA_VERSION};
use serde_json::{json, Value};
use std::process::Command;

fn git(repo: &Utf8Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repo)
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

fn write(repo: &Utf8Path, relative: &str, contents: &str) {
    let path = repo.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn write_graph(repo: &Utf8Path, rule_ids: &[&str]) {
    let rules = rule_ids
        .iter()
        .map(|id| (*id, "active"))
        .collect::<Vec<_>>();
    write_graph_with_statuses(repo, &rules);
}

fn write_graph_with_statuses(repo: &Utf8Path, rules: &[(&str, &str)]) {
    let scope = ScopeId::new("default").unwrap();
    let manifest = Manifest::default_with_scope(scope, RepoPathPrefix::new("."));
    write(
        repo,
        ".provenance/state/manifest.json",
        &serde_json::to_string(&manifest).unwrap(),
    );
    let requirement = json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION,
        "scope_id": "default",
        "id": "req_anchor",
        "statement": "The report classifies missing evidence",
        "status": "active"
    });
    write(
        repo,
        ".provenance/state/scopes/default/requirements/req.jsonl",
        &format!("{requirement}\n"),
    );
    let rules = rules
        .iter()
        .map(|(id, status)| {
            json!({
                "schema_version": SUPPORTED_SCHEMA_VERSION,
                "scope_id": "default",
                "id": id,
                "statement": format!("The {id} behavior is present"),
                "status": status,
                "severity": "high",
                "requirement_ids": ["req_anchor"]
            })
            .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    write(
        repo,
        ".provenance/state/scopes/default/rules/rule.jsonl",
        &format!("{rules}\n"),
    );
}

fn write_implementation_binding(repo: &Utf8Path, retired: Option<bool>) {
    let mut binding = json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION,
        "scope_id": "default",
        "id": "impl_anchor",
        "rule_id": "rule_anchor",
        "declared_by": "fixture",
        "file": "src/lib.rs",
        "symbol": "implements"
    });
    if let Some(retired) = retired {
        binding["retired"] = json!(retired);
    }
    write(
        repo,
        ".provenance/state/scopes/default/implementations/binding.jsonl",
        &format!("{binding}\n"),
    );
}

fn remove_implementation_bindings(repo: &Utf8Path) {
    std::fs::remove_file(
        repo.join(".provenance/state/scopes/default/implementations/binding.jsonl"),
    )
    .unwrap();
}

fn repository(rule_ids: &[&str], source: &str) -> (tempfile::TempDir, Utf8PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let repo = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    write_graph(&repo, rule_ids);
    write(&repo, "src/lib.rs", source);
    (directory, repo)
}

fn commit(repo: &Utf8Path, message: &str) -> String {
    git(repo, &["add", "."]);
    git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-qm", message],
    );
    git(repo, &["rev-parse", "HEAD"])
}

fn envelope(repo: &Utf8Path, base: &str, head: &str) -> super::ReportEnvelope {
    build_envelope(&BuildInput {
        repo,
        scan_path: repo,
        scope: "default",
        base,
        head,
        repository: "quality-sh/provenance",
    })
    .unwrap()
}

fn comparison(envelope: &super::ReportEnvelope, code: &str, rule_id: &str) -> Value {
    let finding = envelope
        .findings
        .iter()
        .find(|finding| finding.code == code && finding.subject.id == rule_id)
        .expect("the expected evidence finding must exist");
    serde_json::to_value(finding.comparison).unwrap()
}

fn has_finding(envelope: &super::ReportEnvelope, code: &str, rule_id: &str) -> bool {
    envelope
        .findings
        .iter()
        .any(|finding| finding.code == code && finding.subject.id == rule_id)
}

#[test]
fn evidence_lost_in_the_range_is_new() {
    let source = "#[rule(\"rule_anchor\")]\nfn implements() {}\n\
                  #[verifies(\"rule_anchor\", examples)]\nfn verifies() {}\n";
    let (_directory, repo) = repository(&["rule_anchor"], source);
    let base = commit(&repo, "Add Rule evidence");
    write(&repo, "src/lib.rs", "fn unrelated() {}\n");
    let head = commit(&repo, "Remove Rule evidence");

    let report = envelope(&repo, &base, &head);

    assert_eq!(
        comparison(&report, "active_rule_missing_implementation", "rule_anchor"),
        "new"
    );
    assert_eq!(
        comparison(&report, "active_rule_missing_verification", "rule_anchor"),
        "new"
    );
}

#[test]
fn evidence_missing_at_both_commits_is_pre_existing() {
    let (_directory, repo) = repository(&["rule_anchor"], "fn before() {}\n");
    let base = commit(&repo, "Add Rule without evidence");
    write(&repo, "src/lib.rs", "fn after() {}\n");
    let head = commit(&repo, "Change unrelated code");

    let report = envelope(&repo, &base, &head);

    assert_eq!(
        comparison(&report, "active_rule_missing_implementation", "rule_anchor"),
        "pre_existing"
    );
    assert_eq!(
        comparison(&report, "active_rule_missing_verification", "rule_anchor"),
        "pre_existing"
    );
}

#[test]
fn rule_added_without_evidence_is_new() {
    let (_directory, repo) = repository(&["rule_anchor"], "fn anchor() {}\n");
    let base = commit(&repo, "Add anchor Rule");
    write_graph(&repo, &["rule_anchor", "rule_added"]);
    let head = commit(&repo, "Add Rule without evidence");

    let report = envelope(&repo, &base, &head);

    assert_eq!(
        comparison(&report, "active_rule_missing_implementation", "rule_added"),
        "new"
    );
    assert_eq!(
        comparison(&report, "active_rule_missing_verification", "rule_added"),
        "new"
    );
}

#[test]
fn removed_implementation_site_is_reported() {
    let source = "#[rule(\"rule_anchor\")]\nfn implements() {}\n";
    let (_directory, repo) = repository(&["rule_anchor"], source);
    let base = commit(&repo, "Add implementation site");
    write(&repo, "src/lib.rs", "fn unrelated() {}\n");
    let head = commit(&repo, "Remove implementation site");

    let report = envelope(&repo, &base, &head);
    let finding = report
        .findings
        .iter()
        .find(|finding| {
            finding.code == "implementation_site_removed" && finding.subject.id == "rule_anchor"
        })
        .expect("the removed implementation site must be reported");

    assert_eq!(finding.comparison, Comparison::New);
    assert_eq!(finding.removed_sites.len(), 1);
    assert_eq!(finding.removed_sites[0].commit, super::CommitRole::Base);
    assert_eq!(finding.removed_sites[0].path, "src/lib.rs");
    assert_eq!(finding.removed_sites[0].line, 1);
    assert_eq!(
        finding.removed_sites[0].role,
        Some(SiteRole::Implementation)
    );
}

#[test]
fn legacy_base_binding_is_evidence_for_comparison() {
    let (_directory, repo) = repository(&["rule_anchor"], "fn before() {}\n");
    write_implementation_binding(&repo, Some(false));
    let base = commit(&repo, "Add historical implementation binding");
    remove_implementation_bindings(&repo);
    write(&repo, "src/lib.rs", "fn after() {}\n");
    let head = commit(&repo, "Remove implementation binding");

    let report = envelope(&repo, &base, &head);

    assert_eq!(
        report.scan.baseline,
        super::BaselineCompatibility::Compatible
    );
    assert_eq!(
        comparison(&report, "active_rule_missing_implementation", "rule_anchor"),
        "new"
    );
}

#[test]
fn incompatible_base_binding_makes_comparisons_uncertain() {
    let (_directory, repo) = repository(&["rule_anchor"], "fn before() {}\n");
    write(
        &repo,
        ".provenance/state/scopes/default/implementations/binding.jsonl",
        "{\"schema_version\":2,\"scope_id\":\"default\",\"id\":\"broken\"}\n",
    );
    let base = commit(&repo, "Add incompatible implementation binding");
    remove_implementation_bindings(&repo);
    write(&repo, "src/lib.rs", "fn after() {}\n");
    let head = commit(&repo, "Remove incompatible binding");

    let report = envelope(&repo, &base, &head);

    assert_eq!(
        report.scan.baseline,
        super::BaselineCompatibility::Incompatible
    );
    assert!(report
        .scan
        .baseline_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("implementations/binding.jsonl")));
    assert_eq!(
        comparison(&report, "active_rule_missing_implementation", "rule_anchor"),
        "uncertain"
    );
    assert_eq!(
        comparison(&report, "active_rule_missing_verification", "rule_anchor"),
        "uncertain"
    );
}

#[test]
fn missing_base_makes_comparisons_uncertain() {
    let directory = tempfile::tempdir().unwrap();
    let repo = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    write(&repo, "src/lib.rs", "fn before() {}\n");
    let base = commit(&repo, "Add source without a graph");
    write_graph(&repo, &["rule_anchor"]);
    let head = commit(&repo, "Add Rule without evidence");

    let report = envelope(&repo, &base, &head);

    assert_eq!(report.scan.baseline, super::BaselineCompatibility::Missing);
    assert_eq!(
        comparison(&report, "active_rule_missing_implementation", "rule_anchor"),
        "uncertain"
    );
    assert_eq!(
        comparison(&report, "active_rule_missing_verification", "rule_anchor"),
        "uncertain"
    );
}

#[test]
fn rule_made_inactive_in_range_has_a_new_current_binding_finding() {
    for status in ["deprecated", "archived"] {
        let source = "#[rule(\"rule_anchor\")]\nfn implements() {}\n";
        let (_directory, repo) = repository(&["rule_anchor"], source);
        let base = commit(&repo, "Add active Rule");
        write_graph_with_statuses(&repo, &[("rule_anchor", status)]);
        let head = commit(&repo, "Make Rule inactive");

        let report = envelope(&repo, &base, &head);

        assert_eq!(
            comparison(&report, "inactive_rule_current_binding", "rule_anchor"),
            "new"
        );
        assert!(!has_finding(
            &report,
            "active_rule_missing_implementation",
            "rule_anchor"
        ));
    }
}

#[test]
fn moved_implementation_anchor_is_not_removed_or_missing() {
    let source = "#[rule(\"rule_anchor\")]\nfn implements() {}\n";
    let (_directory, repo) = repository(&["rule_anchor"], source);
    let base = commit(&repo, "Add implementation anchor");
    write(
        &repo,
        "src/lib.rs",
        "fn unrelated() {}\n#[rule(\"rule_anchor\")]\nfn implements() {}\n",
    );
    let head = commit(&repo, "Move implementation anchor");

    let report = envelope(&repo, &base, &head);

    assert_eq!(
        comparison(&report, "active_rule_missing_verification", "rule_anchor"),
        "pre_existing"
    );
    assert!(!has_finding(
        &report,
        "implementation_site_removed",
        "rule_anchor"
    ));
    assert!(!has_finding(
        &report,
        "active_rule_missing_implementation",
        "rule_anchor"
    ));
}

#[test]
fn scanned_replacement_prevents_removed_or_missing_implementation() {
    let (_directory, repo) = repository(&["rule_anchor"], "fn implements() {}\n");
    write_implementation_binding(&repo, None);
    let base = commit(&repo, "Add typed implementation binding");
    remove_implementation_bindings(&repo);
    write(
        &repo,
        "src/lib.rs",
        "#[rule(\"rule_anchor\")]\nfn implements() {}\n",
    );
    let head = commit(&repo, "Replace typed binding with scanned evidence");

    let report = envelope(&repo, &base, &head);

    assert_eq!(
        comparison(&report, "active_rule_missing_verification", "rule_anchor"),
        "pre_existing"
    );
    assert!(!has_finding(
        &report,
        "implementation_site_removed",
        "rule_anchor"
    ));
    assert!(!has_finding(
        &report,
        "active_rule_missing_implementation",
        "rule_anchor"
    ));
}
