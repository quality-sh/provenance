mod porcelain_authoring_support;

use axum::http::HeaderMap;
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
    fn authenticate(
        &self,
        _: &HeaderMap,
    ) -> Result<(), provenance_core::protocol::failure::OperationFailure> {
        Ok(())
    }

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
        .filter_map(|tool| expected.contains(tool.name.as_ref()).then_some(tool.name.as_ref()))
        .collect::<BTreeSet<_>>();

    assert_eq!(cli, expected);
    assert_eq!(mcp, expected);

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[test]
#[verifies("rule_porcelain_cli_target_action_order", examples)]
#[verifies("rule_porcelain_create_names_new_record", examples)]
fn collection_named_targets_accept_explicit_create_update_and_get() {
    let (_directory, repo) = initialized_repo();

    let created = json_output(&[
        "--repo",
        &repo,
        "sources",
        "create",
        "--type",
        "source",
        "--name",
        "Collection ID",
        "--format",
        "json",
    ]);
    assert_eq!(created["data"]["id"], "sources");

    let updated = json_output(&[
        "sources", "update", "--repo", &repo, "--name", "Changed", "--format", "json",
    ]);
    assert_eq!(updated["data"]["id"], "sources");
    assert_eq!(updated["data"]["name"], "Changed");

    let read = json_output(&["sources", "get", "--repo", &repo, "--format", "json"]);
    assert_eq!(read["record"]["id"], "sources");
}

#[test]
#[verifies("rule_porcelain_cli_target_action_order", examples)]
#[verifies("rule_porcelain_named_domain_actions", examples)]
fn explicit_named_actions_win_for_collection_named_targets() {
    let (_directory, repo) = initialized_repo();
    json_output(&[
        "requirements",
        "create",
        "--repo",
        &repo,
        "--id",
        "req_routing",
        "--statement",
        "The grammar routes explicit target actions.",
        "--format",
        "json",
    ]);
    json_output(&[
        "topics",
        "create",
        "--repo",
        &repo,
        "--id",
        "topics",
        "--requirement-id",
        "req_routing",
        "--title",
        "Routing",
        "--format",
        "json",
    ]);
    json_output(&[
        "questions",
        "create",
        "--repo",
        &repo,
        "--id",
        "questions",
        "--topic-id",
        "topics",
        "--question",
        "Does the grammar select the action?",
        "--method",
        "research",
        "--format",
        "json",
    ]);

    let claimed = json_output(&[
        "topics", "claim", "--repo", &repo, "--actor", "worker", "--format", "json",
    ]);
    assert_eq!(claimed["data"]["claimed_by"], "worker");
    let answered = json_output(&[
        "questions",
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
fn reserved_commands_global_flags_and_legacy_catalog_forms_keep_their_meaning() {
    let (_directory, repo) = initialized_repo();
    provenance().arg("--help").assert().success();
    provenance()
        .args(["check", "--repo", &repo])
        .assert()
        .success();

    let legacy = json_output(&[
        "sources",
        "create",
        "--repo",
        &repo,
        "--id",
        "source_legacy",
        "--name",
        "Legacy",
        "--format",
        "json",
    ]);
    assert_eq!(legacy["data"]["id"], "source_legacy");
    let listed = json_output(&["--repo", &repo, "sources", "list", "--format", "json"]);
    assert!(listed["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|source| source["id"] == json!("source_legacy")));
}
