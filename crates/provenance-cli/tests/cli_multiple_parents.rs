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

#[test]
#[verifies("rule_move_replaces_only_selected_parent", examples)]
#[verifies("rule_supersede_swaps_selected_child_parent", examples)]
fn cli_refusals_preserve_parents_and_a_failed_removal_keeps_the_addition() {
    let (_directory, repo) = parent_repo();
    let before = json_output(&["api", "requirements/req_shared", "--repo", &repo]);
    for new in ["req_missing", "req_child"] {
        provenance()
            .args([
                "req_shared",
                "update",
                "--repo",
                &repo,
                "--actor",
                "author",
                "--replace-parent",
                "req_q",
                "--with-parent",
                new,
            ])
            .assert()
            .failure();
        assert_eq!(
            json_output(&["api", "requirements/req_shared", "--repo", &repo])["data"],
            before["data"]
        );
    }
    provenance()
        .args([
            "req_shared",
            "update",
            "--repo",
            &repo,
            "--actor",
            "author",
            "--replace-parent",
            "req_missing",
            "--with-parent",
            "req_b",
        ])
        .assert()
        .failure();
    let retained = json_output(&["api", "requirements/req_shared", "--repo", &repo]);
    assert_eq!(
        retained["data"]["refines"],
        serde_json::json!(["req_b", "req_p", "req_q"])
    );
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
