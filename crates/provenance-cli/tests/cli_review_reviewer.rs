#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{
    repository_with_actors, request, response, review_command, start_command, Host,
};
use serde_json::{json, Value};
use std::path::Path;

/// Starts a host whose Git configuration is only the given per-user file.
fn start_as(repo: &Path, git_config: &str) -> (Host, tempfile::TempDir) {
    let directory = tempfile::tempdir().unwrap();
    let global = directory.path().join("gitconfig");
    std::fs::write(&global, git_config).unwrap();
    let mut command = review_command(repo);
    command
        .env("GIT_CONFIG_GLOBAL", &global)
        .env("GIT_CONFIG_NOSYSTEM", "1");
    (start_command(command), directory)
}

fn review_config(host: &Host) -> Value {
    serde_json::from_str(
        &request(host, "GET", "/review-config", true)
            .call()
            .unwrap()
            .into_string()
            .unwrap(),
    )
    .unwrap()
}

#[test]
#[verifies("rule_review_host_reports_reviewer", examples)]
fn one_allowed_actor_is_the_reviewer_from_the_git_identity() {
    let repo = repository_with_actors(&["jane_doe_example_com"]);
    let (host, _git) = start_as(repo.path(), "[user]\n\temail = Jane.Doe@example.com\n");

    assert_eq!(
        review_config(&host)["reviewer"],
        json!({"actorId": "jane_doe_example_com", "allowed": true})
    );
}

#[test]
#[verifies("rule_review_host_reports_reviewer", examples)]
fn configured_reviewer_is_reported_when_two_actors_are_allowed() {
    let repo = repository_with_actors(&["release-manager", "maintainer"]);
    let (host, _git) = start_as(
        repo.path(),
        "[user]\n\temail = jane@example.com\n[provenance]\n\treviewer = maintainer\n",
    );

    let config = review_config(&host);
    assert_eq!(
        config["dispositionActorIds"],
        json!(["release-manager", "maintainer"])
    );
    assert_eq!(
        config["reviewer"],
        json!({"actorId": "maintainer", "allowed": true})
    );
}

#[test]
#[verifies("rule_review_host_reports_reviewer", examples)]
fn reviewer_not_on_the_allowlist_is_reported_and_cannot_decide() {
    let repo = repository_with_actors(&["maintainer"]);
    let (host, _git) = start_as(repo.path(), "[provenance]\n\treviewer = visitor\n");

    assert_eq!(
        review_config(&host)["reviewer"],
        json!({"actorId": "visitor", "allowed": false})
    );

    let proposal = pending_proposal(&host, repo.path());
    let refused = decide(&host, &proposal, "visitor");
    assert_eq!(refused.status(), 403);
    let body: Value = serde_json::from_str(&refused.into_string().unwrap()).unwrap();
    assert_eq!(body["error"]["kind"], "access_denied");
    assert_eq!(decide(&host, &proposal, "maintainer").status(), 200);
}

#[test]
#[verifies("rule_review_host_reports_reviewer", examples)]
fn no_git_identity_reports_no_reviewer() {
    let repo = repository_with_actors(&["release-manager", "maintainer"]);
    let (host, _git) = start_as(repo.path(), "");

    let config = review_config(&host);
    assert!(config.get("reviewer").is_none(), "{config}");
}

/// Creates a Requirement and edits it, which opens its review submission.
fn pending_proposal(host: &Host, repo: &Path) -> String {
    let created = host
        .cli()
        .args([
            "req_review",
            "create",
            "--type",
            "requirement",
            "--statement",
            "The review statement applies.",
            "--repo",
        ])
        .arg(repo)
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let read: Value = serde_json::from_str(
        &request(host, "GET", "/requirements/req_review", true)
            .call()
            .unwrap()
            .into_string()
            .unwrap(),
    )
    .unwrap();
    let etag = read["data"]["edit"]["etag"].as_str().unwrap();
    let edited = response(
        request(host, "PATCH", "/requirements/req_review", true)
            .set("Content-Type", "application/json")
            .set("If-Match", etag)
            .send_string(&json!({"actor": "agent", "description": "Revision one."}).to_string()),
    );
    let status = edited.status();
    let edited: Value = serde_json::from_str(&edited.into_string().unwrap()).unwrap();
    assert_eq!(status, 200, "{edited}");
    edited["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn decide(host: &Host, proposal: &str, actor_id: &str) -> ureq::Response {
    response(
        request(
            host,
            "POST",
            &format!("/requirements/req_review/submissions/{proposal}/decide"),
            true,
        )
        .set("Content-Type", "application/json")
        .send_string(
            &json!({
                "actor": {"identity_type": "human", "id": actor_id},
                "decision": "accepted", "rationale": null,
                "canonical_artifact": {"artifact_type": "requirement", "artifact_id": "req_review"},
                "feedback": null, "declared_by": null
            })
            .to_string(),
        ),
    )
}
