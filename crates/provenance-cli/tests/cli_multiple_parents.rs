mod porcelain_authoring_support;
use porcelain_authoring_support::{initialized_repo, json_output, provenance};
use provenance_macros::verifies;

fn parent_repo() -> (tempfile::TempDir, String) {
    let (directory, repo) = initialized_repo();
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
    (directory, repo)
}

#[test]
#[verifies("rule_move_replaces_only_selected_parent", examples)]
#[verifies("rule_move_preserves_identity_and_descendants", examples)]
fn cli_replaces_one_selected_parent_and_can_repeat_the_move() {
    let (_directory, repo) = parent_repo();
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
        json_output(&["api", "requirements/req_child", "--repo", &repo])["data"],
        child["data"]
    );
}

fn replace(repo: &str, old: &str, new: &str) -> assert_cmd::assert::Assert {
    provenance()
        .args([
            "req_shared",
            "update",
            "--repo",
            repo,
            "--actor",
            "author",
            "--replace-parent",
            old,
            "--with-parent",
            new,
        ])
        .assert()
}

fn read(repo: &str, id: &str) -> serde_json::Value {
    json_output(&["api", &format!("requirements/{id}"), "--repo", repo])["data"].clone()
}

fn set_parents(repo: &str, id: &str, parents: &serde_json::Value) {
    json_output(&[
        id,
        "update",
        "--if-match",
        read(repo, id)["edit"]["etag"].as_str().unwrap(),
        "--repo",
        repo,
        "--actor",
        "author",
        "--relationships-json",
        &serde_json::json!({ "refines": parents }).to_string(),
        "--format",
        "json",
    ]);
}

#[test]
#[verifies("rule_move_replaces_only_selected_parent", examples)]
#[verifies("rule_supersede_swaps_selected_child_parent", examples)]
fn cli_refused_new_parents_preserve_parents() {
    let (_directory, repo) = parent_repo();
    let before = read(&repo, "req_shared");
    for new in ["req_missing", "req_child"] {
        replace(&repo, "req_q", new).failure();
        assert_eq!(read(&repo, "req_shared"), before);
    }
}

#[test]
#[verifies("rule_move_replaces_only_selected_parent", examples)]
fn cli_refuses_an_old_parent_that_is_not_a_parent_before_any_write() {
    let (_directory, repo) = parent_repo();
    let before = read(&repo, "req_shared");
    for old in ["req_missing", "req_child"] {
        replace(&repo, old, "req_b")
            .failure()
            .stderr(predicates::str::contains(format!(
                "{old} is not a parent of this Requirement"
            )));
        assert_eq!(read(&repo, "req_shared"), before);
    }
}

#[test]
#[verifies("rule_move_replaces_only_selected_parent", examples)]
#[verifies("rule_move_preserves_identity_and_descendants", examples)]
fn cli_replaces_a_sole_parent() {
    let (_directory, repo) = parent_repo();
    let descendant = read(&repo, "req_child");
    set_parents(&repo, "req_shared", &serde_json::json!(["req_p"]));
    replace(&repo, "req_p", "req_b").success();
    let moved = read(&repo, "req_shared");
    assert_eq!(moved["refines"], serde_json::json!(["req_b"]));
    assert_eq!(moved["id"], "req_shared");
    assert_eq!(read(&repo, "req_child"), descendant);
}

#[test]
#[verifies("rule_supersede_swaps_selected_child_parent", examples)]
fn cli_finishes_a_sole_parent_replacement_after_a_failed_second_step() {
    let (_directory, repo) = parent_repo();
    set_parents(&repo, "req_shared", &serde_json::json!(["req_p"]));
    // A failed removal leaves the added parent next to the sole old parent.
    set_parents(&repo, "req_shared", &serde_json::json!({"add":["req_b"]}));
    assert_eq!(
        read(&repo, "req_shared")["refines"],
        serde_json::json!(["req_b", "req_p"])
    );
    replace(&repo, "req_p", "req_b").success();
    assert_eq!(
        read(&repo, "req_shared")["refines"],
        serde_json::json!(["req_b"])
    );
}

#[test]
#[verifies("rule_move_replaces_only_selected_parent", examples)]
fn cli_refused_replacement_names_the_review_read_command() {
    let (_directory, repo) = parent_repo();
    let stale = read(&repo, "req_shared")["edit"]["etag"]
        .as_str()
        .unwrap()
        .to_owned();
    set_parents(&repo, "req_shared", &serde_json::json!(["req_p"]));
    let before = read(&repo, "req_shared");
    provenance()
        .args([
            "req_shared",
            "update",
            "--repo",
            &repo,
            "--actor",
            "author",
            "--if-match",
            &stale,
            "--replace-parent",
            "req_p",
            "--with-parent",
            "req_b",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "provenance req_shared --view review",
        ));
    assert_eq!(read(&repo, "req_shared"), before);
}

#[test]
#[verifies("rule_move_replaces_only_selected_parent", examples)]
fn cli_replacement_keeps_the_declaration_owner_precondition() {
    let (_directory, repo) = parent_repo();
    let store = provenance_store::state_store::StateStore::new(
        provenance_store::layout::ProvenanceLayout::new(repo.clone()),
    );
    store
        .apply_typed_spec(
            &provenance_core::ScopeId::new("default").unwrap(),
            serde_json::from_value(serde_json::json!({
                "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION.0,
                "spec":"parent_owner", "declared_by":"spec://parent-owner",
                "adopt_unowned":[{"kind":"requirement", "id":"req_shared"}],
                "requirements":[{"key":"shared", "id":"req_shared",
                    "statement":"The system stores records."}]
            }))
            .unwrap(),
        )
        .unwrap();
    let before = json_output(&["api", "requirements/req_shared", "--repo", &repo]);
    for owner in [None, Some("spec://another-owner")] {
        let mut command = provenance();
        command.args([
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
        ]);
        if let Some(owner) = owner {
            command.args(["--declared-by", owner]);
        }
        command.assert().failure();
        assert_eq!(
            json_output(&["api", "requirements/req_shared", "--repo", &repo])["data"],
            before["data"]
        );
    }
    let moved = json_output(&[
        "req_shared",
        "update",
        "--repo",
        &repo,
        "--actor",
        "author",
        "--declared-by",
        "spec://parent-owner",
        "--replace-parent",
        "req_p",
        "--with-parent",
        "req_b",
        "--format",
        "json",
    ]);
    assert_eq!(moved["data"]["declared_by"], "spec://parent-owner");
    assert_eq!(
        moved["data"]["refines"],
        serde_json::json!(["req_b", "req_q"])
    );
}

/// Implementation aid: pins the refusal of a new link to an archived Requirement;
/// no Rule states it yet (provenance-xqpg).
#[test]
fn cli_refuses_an_archived_new_parent() {
    let (_directory, repo) = parent_repo();
    json_output(&[
        "req_b",
        "update",
        "--if-match",
        read(&repo, "req_b")["edit"]["etag"].as_str().unwrap(),
        "--repo",
        &repo,
        "--actor",
        "author",
        "--status",
        "archived",
        "--archived-in-commit-json",
        r#"{"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
        "--format",
        "json",
    ]);
    let before = read(&repo, "req_shared");
    replace(&repo, "req_p", "req_b")
        .failure()
        .stderr(predicates::str::contains("invalid_update"));
    assert_eq!(read(&repo, "req_shared"), before);
}

/// Implementation aid: pins the message and exit status of a finished replacement;
/// no Rule states them yet (provenance-xqpg).
#[test]
fn cli_reports_a_finished_replacement_without_a_write() {
    let (_directory, repo) = parent_repo();
    set_parents(&repo, "req_shared", &serde_json::json!(["req_b", "req_q"]));
    let before = read(&repo, "req_shared");
    replace(&repo, "req_p", "req_b")
        .success()
        .stderr(predicates::str::contains(
            "req_b replaced req_p already; nothing changed",
        ));
    assert_eq!(read(&repo, "req_shared"), before);
}

/// Implementation aid: pins the refusal of a missing old parent; no Rule states it
/// yet (provenance-xqpg).
#[test]
fn cli_refuses_a_missing_old_parent_when_the_new_parent_is_present() {
    let (_directory, repo) = parent_repo();
    set_parents(&repo, "req_shared", &serde_json::json!(["req_b", "req_q"]));
    let before = read(&repo, "req_shared");
    replace(&repo, "req_missing", "req_b")
        .failure()
        .stderr(predicates::str::contains(
            "Requirement req_missing does not exist; nothing changed",
        ));
    assert_eq!(read(&repo, "req_shared"), before);
}
