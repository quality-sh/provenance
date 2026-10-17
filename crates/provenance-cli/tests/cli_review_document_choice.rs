mod porcelain_authoring_support;
mod review_host_support;

use porcelain_authoring_support::{initialized_repo, json_output, provenance};
use provenance_macros::verifies;
use review_host_support::Host;
use serde_json::Value;

fn requirement(repo: &str, id: &str, parent: Option<&str>) {
    let mut args = vec![
        "requirements",
        "create",
        "--id",
        id,
        "--statement",
        "The reviewer can read the task.",
        "--repo",
        repo,
        "--format",
        "json",
    ];
    if let Some(parent) = parent {
        args.extend(["--refines", parent]);
    }
    json_output(&args);
}

fn nested(repo: &str) {
    requirement(repo, "req_a_root", None);
    requirement(repo, "req_m_middle", Some("req_a_root"));
    requirement(repo, "req_z_leaf", Some("req_m_middle"));
    json_output(&[
        "topics",
        "create",
        "--id",
        "topic_leaf",
        "--requirement-id",
        "req_z_leaf",
        "--title",
        "Review the task",
        "--repo",
        repo,
        "--format",
        "json",
    ]);
    json_output(&[
        "questions",
        "create",
        "--id",
        "question_leaf",
        "--topic-id",
        "topic_leaf",
        "--question",
        "Which task needs review?",
        "--resolution-method",
        "research",
        "--repo",
        repo,
        "--format",
        "json",
    ]);
}

fn link(host: &Host, repo: &str, id: &str) -> url::Url {
    let output = host
        .cli()
        .args([
            id,
            "--review-link",
            "--no-open",
            "--format",
            "json",
            "--repo",
            repo,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    url::Url::parse(value["review_url"].as_str().unwrap()).unwrap()
}

fn assert_target(url: &url::Url, root: &str, focus: Option<&str>) {
    let pairs = url
        .query_pairs()
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(pairs.get("root").map(|s| s.as_ref()), Some(root));
    assert_eq!(pairs.get("focus").map(|s| s.as_ref()), focus);
}

#[test]
#[verifies("rule_review_document_choice", examples)]
fn nested_requirements_each_open_their_own_document() {
    let (_directory, repo) = initialized_repo();
    nested(&repo);
    let host = review_host_support::start(std::path::Path::new(&repo));
    for id in ["req_a_root", "req_m_middle", "req_z_leaf"] {
        assert_target(&link(&host, &repo, id), id, None);
    }
}

#[test]
#[verifies("rule_review_document_choice", examples)]
fn nested_rule_and_question_open_the_leaf_document() {
    let (_directory, repo) = initialized_repo();
    nested(&repo);
    json_output(&[
        "rules",
        "create",
        "--id",
        "rule_leaf",
        "--requirement-id",
        "req_z_leaf",
        "--statement",
        "The reviewer can read the rule.",
        "--repo",
        &repo,
        "--format",
        "json",
    ]);
    let host = review_host_support::start(std::path::Path::new(&repo));
    for id in ["rule_leaf", "question_leaf"] {
        assert_target(&link(&host, &repo, id), "req_z_leaf", Some(id));
    }
}

#[test]
#[verifies("rule_review_document_choice", examples)]
fn nested_write_returns_a_link_without_a_warning() {
    let (_directory, repo) = initialized_repo();
    nested(&repo);
    let host = review_host_support::start(std::path::Path::new(&repo));
    let output = host
        .cli()
        .args([
            "rules",
            "create",
            "--id",
            "rule_write",
            "--requirement-id",
            "req_z_leaf",
            "--statement",
            "The reviewer can read the rule.",
            "--repo",
            &repo,
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let url = url::Url::parse(value["data"]["review_url"].as_str().unwrap()).unwrap();
    assert_target(&url, "req_z_leaf", Some("rule_write"));
}

#[test]
#[verifies("rule_review_document_choice", examples)]
fn single_requirement_keeps_its_document() {
    let (_directory, repo) = initialized_repo();
    requirement(&repo, "req_only", None);
    let host = review_host_support::start(std::path::Path::new(&repo));
    assert_target(&link(&host, &repo, "req_only"), "req_only", None);
}

#[test]
#[verifies("rule_review_document_choice", examples)]
fn equal_distances_use_the_lowest_id_regardless_of_creation_order() {
    let (_directory, repo) = initialized_repo();
    requirement(&repo, "req_z", None);
    requirement(&repo, "req_a", None);
    provenance()
        .args([
            "rules",
            "create",
            "--id",
            "rule_shared",
            "--requirement-id",
            "req_z",
            "--requirement-id",
            "req_a",
            "--statement",
            "The reviewer can read the rule.",
            "--repo",
            &repo,
            "--format",
            "json",
        ])
        .assert()
        .success();
    let host = review_host_support::start(std::path::Path::new(&repo));
    assert_target(
        &link(&host, &repo, "rule_shared"),
        "req_a",
        Some("rule_shared"),
    );
}
