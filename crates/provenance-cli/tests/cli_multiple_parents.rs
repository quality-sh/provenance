#![cfg(feature = "test-fixture")]
mod porcelain_authoring_support;
use porcelain_authoring_support::{initialized_repo, json_output};
use provenance_macros::verifies;

#[test]
#[verifies("rule_move_replaces_only_selected_parent", examples)]
#[verifies("rule_move_preserves_identity_and_descendants", examples)]
fn cli_replaces_one_selected_parent_and_can_repeat_the_move() {
    let (_directory, repo) = initialized_repo();
    for id in ["req_p", "req_q", "req_b", "req_shared", "req_child"] {
        json_output(&[
            id,
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "The system stores records.",
            "--format",
            "json",
        ]);
    }
    let shared = json_output(&["api", "requirements/req_shared", "--repo", &repo]);
    json_output(&[
        "req_shared",
        "update",
        "--if-match",
        shared["data"]["edit"]["etag"].as_str().unwrap(),
        "--repo",
        &repo,
        "--actor",
        "author",
        "--relationships-json",
        r#"{"refines":["req_p","req_q"]}"#,
        "--format",
        "json",
    ]);
    let child = json_output(&["api", "requirements/req_child", "--repo", &repo]);
    json_output(&[
        "req_child",
        "update",
        "--if-match",
        child["data"]["edit"]["etag"].as_str().unwrap(),
        "--repo",
        &repo,
        "--actor",
        "author",
        "--relationships-json",
        r#"{"refines":["req_shared"]}"#,
        "--format",
        "json",
    ]);
    let child = json_output(&["api", "requirements/req_child", "--repo", &repo]);
    for _ in 0..2 {
        let moved = json_output(&[
            "req_shared",
            "update",
            "--repo",
            &repo,
            "--actor",
            "author",
            "--replace-parent",
            "req_p",
            "--with-parent",
            "req_b",
            "--format",
            "json",
        ]);
        assert_eq!(
            moved["data"]["refines"],
            serde_json::json!(["req_b", "req_q"])
        );
        assert_eq!(moved["data"]["id"], "req_shared");
    }
    assert_eq!(
        json_output(&["api", "requirements/req_child", "--repo", &repo]),
        child
    );
}
