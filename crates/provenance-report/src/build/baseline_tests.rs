//! Base evidence that is retired, unreadable or replaced keeps the report
//! comparisons honest.

use super::test_repo::commit;
use super::tests::{
    comparison, envelope, has_finding, remove_implementation_bindings, repository, write,
    write_graph, write_graph_with_statuses, write_implementation_binding,
};
use crate::envelope::{BaselineCompatibility, Completeness};

#[test]
fn retired_base_binding_is_not_evidence_for_comparison() {
    let (_directory, repo) = repository(&["rule_anchor"], "fn before() {}\n");
    write_implementation_binding(&repo, Some(true));
    let base = commit(&repo, "Add retired implementation binding");
    remove_implementation_bindings(&repo);
    write(&repo, "src/lib.rs", "fn after() {}\n");
    let head = commit(&repo, "Remove retired implementation binding");

    let report = envelope(&repo, &base, &head);

    assert_eq!(report.scan.baseline, BaselineCompatibility::Compatible);
    assert_eq!(
        comparison(&report, "active_rule_missing_implementation", "rule_anchor"),
        "pre_existing"
    );
}

#[test]
fn unreadable_base_binding_keeps_graph_and_source_findings() {
    let source = "#[rule(\"rule_anchor\")]\nfn implements() {}\n";
    let (_directory, repo) = repository(&["rule_anchor"], source);
    write(
        &repo,
        ".provenance/state/scopes/default/implementations/binding.jsonl",
        "{\"schema_version\":2,\"scope_id\":\"default\",\"id\":\"broken\"}\n",
    );
    let base = commit(&repo, "Add incompatible implementation binding");
    remove_implementation_bindings(&repo);
    let rules = repo.join(".provenance/state/scopes/default/rules/rule.jsonl");
    let changed = std::fs::read_to_string(&rules)
        .unwrap()
        .replace("behavior is present", "behavior is changed");
    std::fs::write(&rules, changed).unwrap();
    write(&repo, "src/lib.rs", "fn implements() {}\n");
    let head = commit(&repo, "Change the Rule and remove its marker");

    let report = envelope(&repo, &base, &head);

    assert_eq!(report.scan.baseline, BaselineCompatibility::Incompatible);
    assert!(report
        .scan
        .baseline_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("implementations/binding.jsonl")));
    assert!(!report.graph_changes.is_empty());
    assert_eq!(
        comparison(&report, "requirement_statement_changed", "rule_anchor"),
        "uncertain"
    );
    assert_eq!(
        comparison(&report, "implementation_site_removed", "rule_anchor"),
        "uncertain"
    );
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
fn typed_replacement_of_a_scanned_site_is_not_removed() {
    let source = "#[rule(\"rule_anchor\")]\nfn implements() {}\n";
    let (_directory, repo) = repository(&["rule_anchor"], source);
    let base = commit(&repo, "Add scanned implementation");
    write(&repo, "src/lib.rs", "fn implements() {}\n");
    write_implementation_binding(&repo, None);
    let head = commit(&repo, "Replace scanned evidence with a typed binding");

    let report = envelope(&repo, &base, &head);

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
fn unreadable_base_source_keeps_an_incomplete_report() {
    let (_directory, repo) = repository(&["rule_anchor"], "fn before() {}\n");
    std::fs::write(repo.join("src/legacy.rs"), [0xff, 0xfe, b'\n']).unwrap();
    let base = commit(&repo, "Add a file that is not UTF-8");
    std::fs::remove_file(repo.join("src/legacy.rs")).unwrap();
    let head = commit(&repo, "Remove the file that is not UTF-8");
    write(&repo, "notes.txt", "uncommitted\n");

    let report = envelope(&repo, &base, &head);

    assert_eq!(report.scan.completeness, Completeness::Incomplete);
    assert_eq!(report.scan.baseline, BaselineCompatibility::Incompatible);
    assert!(report
        .scan
        .baseline_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("source files at base commit")));
}

#[test]
fn unreadable_base_source_on_a_complete_scan_keeps_the_other_findings() {
    let (_directory, repo) = repository(&["rule_anchor"], "fn before() {}\n");
    std::fs::write(repo.join("src/legacy.rs"), [0xff, 0xfe, b'\n']).unwrap();
    let base = commit(&repo, "Add a file that is not UTF-8");
    std::fs::remove_file(repo.join("src/legacy.rs")).unwrap();
    let rules = repo.join(".provenance/state/scopes/default/rules/rule.jsonl");
    let changed = std::fs::read_to_string(&rules)
        .unwrap()
        .replace("behavior is present", "behavior is changed");
    std::fs::write(&rules, changed).unwrap();
    let head = commit(&repo, "Remove the file and change the Rule");

    let report = envelope(&repo, &base, &head);

    assert_eq!(report.scan.completeness, Completeness::Complete);
    assert_eq!(report.scan.baseline, BaselineCompatibility::Incompatible);
    assert!(report
        .scan
        .baseline_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("source files at base commit")));
    assert!(!report.graph_changes.is_empty());
    assert_eq!(
        comparison(&report, "requirement_statement_changed", "rule_anchor"),
        "uncertain"
    );
    assert_eq!(
        comparison(&report, "active_rule_missing_implementation", "rule_anchor"),
        "uncertain"
    );
}

#[test]
fn rule_made_active_in_range_has_new_missing_evidence() {
    for status in ["draft", "review"] {
        assert_activated_rule_is_new(status);
    }
}

#[test]
fn deprecated_rule_made_active_again_has_new_missing_evidence() {
    assert_activated_rule_is_new("deprecated");
}

fn assert_activated_rule_is_new(base_status: &str) {
    let (_directory, repo) = repository(&["rule_anchor"], "fn anchor() {}\n");
    write_graph_with_statuses(
        &repo,
        &[("rule_anchor", "active"), ("rule_later", base_status)],
    );
    let base = commit(&repo, "Add a Rule that is not active");
    write_graph(&repo, &["rule_anchor", "rule_later"]);
    let head = commit(&repo, "Make the Rule active");

    let report = envelope(&repo, &base, &head);

    assert_eq!(
        comparison(&report, "active_rule_missing_implementation", "rule_later"),
        "new",
        "base status {base_status}"
    );
    assert_eq!(
        comparison(&report, "active_rule_missing_verification", "rule_later"),
        "new",
        "base status {base_status}"
    );
    assert_eq!(
        comparison(&report, "active_rule_missing_implementation", "rule_anchor"),
        "pre_existing"
    );
}
