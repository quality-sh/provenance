#[path = "query_support/fixtures.rs"]
mod fixtures;

use fixtures::{init_repo, resource_id, sdk, shared_rule_spec, verify_rule, Ids};
use serde_json::{json, Value};
use std::path::Path;

fn git(repo: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn commit(repo: &Path, message: &str) -> String {
    git(repo, &["add", "-A"]);
    git(
        repo,
        &[
            "-c",
            "user.name=Provenance Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-m",
            message,
        ],
    );
    git(repo, &["rev-parse", "HEAD"])
}

/// A repository whose evidence files are tracked across two commits.
fn tracked_repo() -> (tempfile::TempDir, Ids, String) {
    let directory = init_repo();
    let path = directory.path();
    let repo = path.to_str().unwrap();
    git(path, &["init"]);
    std::fs::write(
        path.join("share-links.ts"),
        "export function createShareLink() {}\n",
    )
    .unwrap();
    std::fs::write(
        path.join("share-links.test.ts"),
        "export const expiry = 30;\n",
    )
    .unwrap();
    let applied = sdk(repo, "apply", &shared_rule_spec());
    let ids = Ids {
        source: resource_id(&applied, "source", "retention"),
        sharing: resource_id(&applied, "requirement", "sharing"),
        sessions: resource_id(&applied, "requirement", "sessions"),
        rule: resource_id(&applied, "rule", "expiry"),
    };
    verify_rule(repo, &ids.rule);
    let base = commit(path, "base");
    std::fs::write(
        path.join("share-links.ts"),
        "export function createShareLink() { return 14; }\n",
    )
    .unwrap();
    std::fs::write(
        path.join("share-links.test.ts"),
        "export const expiry = 14;\n",
    )
    .unwrap();
    commit(path, "head");
    (directory, ids, base)
}

fn subjects(answer: &Value) -> Vec<String> {
    answer["sites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|site| site["subject_id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn stale_reports_the_evidence_sites_a_commit_range_disturbed() {
    let (directory, ids, base) = tracked_repo();
    let repo = directory.path().to_str().unwrap();

    let answer = sdk(repo, "stale", &json!({"base": base.as_str()}));

    assert_eq!(answer["protocol_version"], 6);
    assert_eq!(answer["operation"], "stale");
    assert_eq!(answer["base"], base);
    assert_eq!(answer["has_more"], false);
    assert!(
        subjects(&answer).iter().all(|subject| *subject == ids.rule),
        "every disturbed site should name the rule: {answer}"
    );
    assert!(answer["sites"]
        .as_array()
        .unwrap()
        .iter()
        .any(|site| site["file_path"] == "share-links.ts" && site["state"] == "touched"));
    assert!(answer["sites"]
        .as_array()
        .unwrap()
        .iter()
        .any(|site| site["file_path"] == "share-links.test.ts" && site["state"] == "touched"));
    assert_eq!(answer["summary"]["touched"], 2);
}

#[test]
fn stale_reads_only_the_rules_the_caller_names() {
    let (directory, _ids, base) = tracked_repo();
    let repo = directory.path().to_str().unwrap();

    let answer = sdk(
        repo,
        "stale",
        &json!({"base": base, "rules": ["rule_nobody_declared"]}),
    );

    assert_eq!(answer["sites"], json!([]));
    assert_eq!(answer["summary"]["total_sites"], 0);
}

#[test]
fn stale_bounds_its_page_and_says_when_more_remain() {
    let (directory, _ids, base) = tracked_repo();
    let repo = directory.path().to_str().unwrap();

    let answer = sdk(repo, "stale", &json!({"base": base, "limit": 1}));

    assert_eq!(answer["sites"].as_array().unwrap().len(), 1);
    assert_eq!(answer["has_more"], true);
    assert_eq!(answer["summary"]["total_sites"], 2);
}

#[test]
fn evidence_reports_stale_only_when_the_caller_supplies_a_diff() {
    let (directory, ids, base) = tracked_repo();
    let repo = directory.path().to_str().unwrap();

    let without = sdk(repo, "evidence", &json!({"rule": ids.rule.as_str()}));
    assert_eq!(without["stale"], json!(null));

    let with = sdk(
        repo,
        "evidence",
        &json!({"rule": ids.rule, "base": base.as_str()}),
    );
    assert_eq!(with["stale"]["base"], base);
    assert_eq!(with["stale"]["sites"].as_array().unwrap().len(), 2);
}
