mod porcelain_authoring_support;

use porcelain_authoring_support::{
    allow_reviewer, initialized_repo, json_output, json_stdin_output, local_host,
    local_host_with_identity,
};
use provenance_macros::verifies;
use provenance_transport::local_host::LocalHostRegistration;
use serde_json::json;
use std::net::TcpListener;

#[test]
#[verifies("rule_cli_record_review_action_returns_url", examples)]
fn write_output_links_to_the_containing_requirement() {
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
}

#[test]
#[verifies("rule_cli_record_review_action_returns_url", examples)]
fn explicit_read_links_to_the_containing_requirement() {
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
    json_output(&[
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
    let host = local_host(&repo);
    let expected = format!("{}/?root=req_link&focus=rule_link", host.endpoint);

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
fn a_stale_listener_does_not_produce_a_link() {
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
    let _registration =
        LocalHostRegistration::publish(std::path::Path::new(&repo), "default", &endpoint, "local")
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

fn create_requirement(repo: &str, id: &str) -> serde_json::Value {
    json_output(&[
        id,
        "create",
        "--type",
        "requirement",
        "--repo",
        repo,
        "--statement",
        "The review action keeps its link.",
        "--format",
        "json",
    ])
}

fn update_payload() -> serde_json::Value {
    json!({"actor":"agent","description":"Updated review text."})
}

fn withdraw_payload() -> serde_json::Value {
    json!({"actor":"agent","declared_by":null,"reason":null})
}

fn submission_payload() -> serde_json::Value {
    json!({
        "actor":"agent", "title":"Review", "summary":"Review the updated record.",
        "source_ids":[], "evidence_references":[], "builds_on":[]
    })
}

fn decision_payload() -> serde_json::Value {
    json!({
        "actor":{"identity_type":"human","id":"reviewer"}, "decision":"accepted",
        "rationale":null,
        "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_flow"},
        "feedback":null, "declared_by":null
    })
}

fn pending_submission(repo: &str) -> (serde_json::Value, String) {
    let created = create_requirement(repo, "req_flow");
    let proposal = created["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned();
    (created, proposal)
}

fn ready_to_submit(repo: &str) {
    let (_, proposal) = pending_submission(repo);
    json_stdin_output(
        &[
            "requirements",
            "req_flow",
            "submissions",
            &proposal,
            "withdraw",
            "--repo",
            repo,
            "--stdin",
            "--format",
            "json",
        ],
        &withdraw_payload(),
    );
}

fn assert_review_link(output: &serde_json::Value, root: &str) {
    assert_eq!(output["data"]["review_url"], root);
}

#[test]
#[verifies("rule_cli_record_review_action_returns_url", examples)]
fn create_returns_the_review_link() {
    let (_directory, repo) = initialized_repo();
    let host = local_host(&repo);

    let created = create_requirement(&repo, "req_flow");

    assert_review_link(&created, &format!("{}/?root=req_flow", host.endpoint));
}

#[test]
#[verifies("rule_cli_record_review_action_returns_url", examples)]
fn update_returns_the_review_link() {
    let (_directory, repo) = initialized_repo();
    let created = create_requirement(&repo, "req_flow");
    let etag = created["data"]["edit"]["etag"].as_str().unwrap();
    let host = local_host(&repo);
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
        &update_payload(),
    );

    assert_review_link(&updated, &format!("{}/?root=req_flow", host.endpoint));
}

#[test]
#[verifies("rule_cli_record_review_action_returns_url", examples)]
fn withdraw_returns_the_review_link() {
    let (_directory, repo) = initialized_repo();
    let (_, proposal) = pending_submission(&repo);
    let host = local_host(&repo);
    let withdrawn = json_stdin_output(
        &[
            "requirements",
            "req_flow",
            "submissions",
            &proposal,
            "withdraw",
            "--repo",
            &repo,
            "--stdin",
            "--format",
            "json",
        ],
        &withdraw_payload(),
    );

    assert_review_link(&withdrawn, &format!("{}/?root=req_flow", host.endpoint));
}

#[test]
#[verifies("rule_cli_record_review_action_returns_url", examples)]
fn submit_returns_the_review_link() {
    let (_directory, repo) = initialized_repo();
    ready_to_submit(&repo);
    let host = local_host(&repo);
    let submitted = json_stdin_output(
        &[
            "req_flow", "submit", "--repo", &repo, "--stdin", "--format", "json",
        ],
        &submission_payload(),
    );

    assert_review_link(&submitted, &format!("{}/?root=req_flow", host.endpoint));
}

#[test]
#[verifies("rule_cli_record_review_action_returns_url", examples)]
fn decide_returns_the_review_link() {
    let (_directory, repo) = initialized_repo();
    allow_reviewer(&repo);
    let (_, proposal) = pending_submission(&repo);
    let host = local_host(&repo);
    let decided = json_stdin_output(
        &[
            "requirements",
            "req_flow",
            "submissions",
            &proposal,
            "decide",
            "--repo",
            &repo,
            "--stdin",
            "--format",
            "json",
        ],
        &decision_payload(),
    );

    assert_review_link(&decided, &format!("{}/?root=req_flow", host.endpoint));
}
