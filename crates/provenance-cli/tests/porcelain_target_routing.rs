mod porcelain_authoring_support;

use provenance_macros::verifies;
use provenance_store::operations::catalog::{
    ContextResolver, ExecutionNeeds, PreparedContext, RequestedContext,
};
use provenance_transport::{HostAccess, StatementHost};
use rmcp::ServiceExt as _;
use serde_json::json;
use std::{collections::BTreeSet, sync::Arc};

use porcelain_authoring_support::{initialized_repo, json_output, provenance};

struct AdvertiseAll;

impl ContextResolver for AdvertiseAll {
    fn authorize(
        &self,
        _: &'static str,
        _: &RequestedContext,
    ) -> Result<(), provenance_core::protocol::failure::OperationFailure> {
        Ok(())
    }

    fn prepare(
        &self,
        _: &'static str,
        _: RequestedContext,
        _: ExecutionNeeds,
    ) -> Result<PreparedContext, provenance_core::protocol::failure::OperationFailure> {
        Err(provenance_core::protocol::failure::OperationFailure::UnavailableNeeds)
    }
}

impl HostAccess for AdvertiseAll {
    fn advertises(&self, _: &str) -> bool {
        true
    }

    fn bound_identity(&self) -> Option<(String, String)> {
        None
    }
}

#[tokio::test]
#[verifies("rule_porcelain_action_names_match", examples)]
async fn target_first_action_names_match_on_the_live_cli_and_mcp_surfaces() {
    let expected = BTreeSet::from(["answer", "claim", "create", "release", "submit", "update"]);
    let cli = expected
        .iter()
        .filter_map(|action| {
            provenance()
                .args(["record_target", action, "--help"])
                .output()
                .unwrap()
                .status
                .success()
                .then_some(*action)
        })
        .collect::<BTreeSet<_>>();

    let host = StatementHost::with_access(Arc::new(AdvertiseAll));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    let mcp = tools
        .iter()
        .filter_map(|tool| {
            expected
                .contains(tool.name.as_ref())
                .then_some(tool.name.as_ref())
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(cli, expected);
    assert_eq!(mcp, expected);

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[test]
#[verifies("rule_porcelain_cli_target_action_order", examples)]
#[verifies("rule_porcelain_named_domain_actions", examples)]
fn named_actions_work_for_ordinary_targets() {
    let (_directory, repo) = initialized_repo();
    json_output(&[
        "req_routing",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The grammar routes explicit target actions.",
        "--format",
        "json",
    ]);
    json_output(&[
        "topic_routing",
        "create",
        "--type",
        "topic",
        "--repo",
        &repo,
        "--requirement-id",
        "req_routing",
        "--title",
        "Routing",
        "--format",
        "json",
    ]);
    json_output(&[
        "question_routing",
        "create",
        "--type",
        "question",
        "--repo",
        &repo,
        "--topic-id",
        "topic_routing",
        "--question",
        "Does the grammar select the action?",
        "--method",
        "research",
        "--format",
        "json",
    ]);

    let claimed = json_output(&[
        "topic_routing",
        "claim",
        "--repo",
        &repo,
        "--actor",
        "worker",
        "--format",
        "json",
    ]);
    assert_eq!(claimed["data"]["claimed_by"], "worker");
    let question_claimed = json_output(&[
        "question_routing",
        "claim",
        "--repo",
        &repo,
        "--actor",
        "worker",
        "--format",
        "json",
    ]);
    assert_eq!(question_claimed["data"]["claimed_by"], "worker");
    let question_released = json_output(&[
        "question_routing",
        "release",
        "--repo",
        &repo,
        "--actor",
        "worker",
        "--format",
        "json",
    ]);
    assert!(question_released["data"]["claimed_by"].is_null());
    let answered = json_output(&[
        "question_routing",
        "answer",
        "--repo",
        &repo,
        "--answer",
        "Yes.",
        "--format",
        "json",
    ]);
    assert_eq!(answered["data"]["status"], "answered");
}

#[test]
fn reserved_commands_global_flags_and_api_forms_keep_their_meaning() {
    let (_directory, repo) = initialized_repo();
    provenance().arg("--help").assert().success();
    provenance()
        .args(["check", "--repo", &repo])
        .assert()
        .success();

    let created = json_output(&[
        "source_legacy",
        "create",
        "--type",
        "source",
        "--repo",
        &repo,
        "--name",
        "Legacy",
        "--format",
        "json",
    ]);
    assert_eq!(created["data"]["id"], "source_legacy");
    let listed = json_output(&["--repo", &repo, "api", "sources", "--format", "json"]);
    assert!(listed["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|source| source["id"] == json!("source_legacy")));
}
