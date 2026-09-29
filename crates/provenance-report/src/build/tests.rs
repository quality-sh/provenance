use super::{build_envelope, BuildInput};
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
    let rules = rule_ids
        .iter()
        .map(|id| {
            json!({
                "schema_version": SUPPORTED_SCHEMA_VERSION,
                "scope_id": "default",
                "id": id,
                "statement": format!("The {id} behavior is present"),
                "status": "active",
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

    assert_eq!(finding.comparison, super::Comparison::New);
    assert_eq!(finding.removed_sites.len(), 1);
    assert_eq!(finding.removed_sites[0].commit, super::CommitRole::Base);
    assert_eq!(finding.removed_sites[0].path, "src/lib.rs");
    assert_eq!(finding.removed_sites[0].line, 1);
    assert_eq!(
        finding.removed_sites[0].role,
        Some(super::SiteRole::Implementation)
    );
}
