mod porcelain_authoring_support;

use porcelain_authoring_support::{
    initialized_repo, json_output, json_stdin_output, local_host, local_host_with_identity,
    provenance, write_local_host_fixture,
};
use provenance_macros::verifies;
use provenance_transport::local_host::LocalHostRegistration;
use serde_json::json;
use std::net::TcpListener;

#[test]
#[verifies("rule_agent_review_request_includes_link", examples)]
fn write_output_explains_how_to_start_a_missing_review_host() {
    let (_directory, repo) = initialized_repo();
    let created = json_output(&[
        "req_link",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The agent gives the reviewer a link.",
        "--format",
        "json",
    ]);

    assert!(created["data"].get("review_url").is_none());
    assert!(created["data"]["review_message"]
        .as_str()
        .unwrap()
        .contains("provenance review"));

    provenance()
        .args([
            "req_readable_link",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "The readable output explains how to start review.",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("provenance review --repo"));
}

#[test]
#[verifies("rule_agent_review_request_includes_link", examples)]
fn write_and_explicit_read_link_to_the_containing_requirement() {
    let (_directory, repo) = initialized_repo();
    json_output(&[
        "req_link",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The agent gives the reviewer a link.",
        "--format",
        "json",
    ]);
    let host = local_host(&repo);

    let created = json_output(&[
        "rule_link",
        "create",
        "--type",
        "rule",
        "--repo",
        &repo,
        "--statement",
        "The review output includes a link.",
        "--requirement-id",
        "req_link",
        "--format",
        "json",
    ]);
    let expected = format!("{}/?root=req_link&focus=rule_link", host.endpoint);
    assert_eq!(created["data"]["review_url"], expected);

    let link = json_output(&[
        "rule_link",
        "get",
        "--repo",
        &repo,
        "--review-link",
        "--format",
        "json",
    ]);
    assert_eq!(link["review_url"], expected);
}

#[test]
#[verifies("rule_review_link_opens_repository_host_only", examples)]
fn stale_listener_and_invalid_runtime_records_do_not_produce_links() {
    for endpoint in [
        "https://127.0.0.1:1234",
        "http://127.0.0.1:1234/path",
        "http://user@127.0.0.1:1234",
        "not a URL",
    ] {
        let (_directory, repo) = initialized_repo();
        json_output(&[
            "req_link",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "The agent gives the reviewer a safe link.",
            "--format",
            "json",
        ]);
        write_local_host_fixture(&repo, endpoint);
        let link = json_output(&[
            "req_link",
            "get",
            "--repo",
            &repo,
            "--review-link",
            "--format",
            "json",
        ]);
        assert!(link["review_url"].is_null());
        assert!(link["message"]
            .as_str()
            .unwrap()
            .contains("provenance review"));
    }

    let (_directory, repo) = initialized_repo();
    json_output(&[
        "req_link",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The agent gives the reviewer a safe link.",
        "--format",
        "json",
    ]);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let _registration = LocalHostRegistration::publish(
        std::path::Path::new(&repo),
        "default",
        &endpoint,
        "local",
    )
    .unwrap();
    let link = json_output(&[
        "req_link",
        "get",
        "--repo",
        &repo,
        "--review-link",
        "--format",
        "json",
    ]);
    assert!(link["review_url"].is_null());
}

#[test]
#[verifies("rule_review_link_opens_repository_host_only", examples)]
fn identity_mismatches_do_not_produce_links() {
    for (field, mismatch) in [
        ("repositoryId", "other"),
        ("scope", "other"),
        ("instanceNonce", "other"),
    ] {
        let (_directory, repo) = initialized_repo();
        json_output(&[
            "req_link",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "The agent gives the reviewer a safe link.",
            "--format",
            "json",
        ]);
        let _host = local_host_with_identity(&repo, |identity| {
            let mut response = serde_json::to_value(identity).unwrap();
            response[field] = mismatch.into();
            response
        });
        let link = json_output(&[
            "req_link",
            "get",
            "--repo",
            &repo,
            "--review-link",
            "--format",
            "json",
        ]);
        assert!(link["review_url"].is_null());
    }
}

#[test]
#[verifies("rule_agent_review_request_includes_link", examples)]
fn explicit_link_read_explains_how_to_start_the_host() {
    let (_directory, repo) = initialized_repo();
    json_output(&[
        "req_link",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The agent gives the reviewer a link.",
        "--format",
        "json",
    ]);

    let output = json_output(&[
        "req_link",
        "get",
        "--repo",
        &repo,
        "--review-link",
        "--format",
        "json",
    ]);
    assert!(output["review_url"].is_null());
    assert!(output["message"]
        .as_str()
        .unwrap()
        .contains("provenance review"));
}

fn allow_reviewer(repo: &str) {
    let layout = provenance_store::layout::ProvenanceLayout::new(repo);
    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

#[test]
#[verifies("rule_agent_review_request_includes_link", examples)]
fn update_submit_withdraw_and_decide_outputs_keep_the_review_link() {
    let (_directory, repo) = initialized_repo();
    allow_reviewer(&repo);
    let host = local_host(&repo);
    let expected = format!("{}/?root=req_flow", host.endpoint);
    let created = json_output(&[
        "req_flow",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The review flow keeps its link.",
        "--format",
        "json",
    ]);
    assert_eq!(created["data"]["review_url"], expected);
    let etag = created["data"]["edit"]["etag"].as_str().unwrap();
    let updated = json_stdin_output(
        &[
            "requirements",
            "req_flow",
            "update",
            "--repo",
            &repo,
            "--if-match",
            etag,
            "--stdin",
            "--format",
            "json",
        ],
        &json!({"actor":"agent","description":"Updated review text."}),
    );
    assert_eq!(updated["data"]["review_url"], expected);
    let automatic = updated["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap();
    let withdrawn = json_stdin_output(
        &[
            "requirements",
            "req_flow",
            "submissions",
            automatic,
            "withdraw",
            "--repo",
            &repo,
            "--stdin",
            "--format",
            "json",
        ],
        &json!({"actor":"agent","declared_by":null,"reason":null}),
    );
    assert_eq!(withdrawn["data"]["review_url"], expected);
    let submitted = json_stdin_output(
        &[
            "req_flow", "submit", "--repo", &repo, "--stdin", "--format", "json",
        ],
        &json!({
            "actor":"agent", "title":"Review", "summary":"Review the updated record.",
            "source_ids":[], "evidence_references":[], "builds_on":[]
        }),
    );
    assert_eq!(submitted["data"]["review_url"], expected);
    let proposal = submitted["data"]["proposal_id"].as_str().unwrap();
    let decided = json_stdin_output(
        &[
            "requirements",
            "req_flow",
            "submissions",
            proposal,
            "decide",
            "--repo",
            &repo,
            "--stdin",
            "--format",
            "json",
        ],
        &json!({
            "actor":{"identity_type":"human","id":"reviewer"}, "decision":"accepted",
            "rationale":null,
            "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_flow"},
            "feedback":null, "declared_by":null
        }),
    );
    assert_eq!(decided["data"]["review_url"], expected);
}
