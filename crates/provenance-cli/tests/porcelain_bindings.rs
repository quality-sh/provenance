use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::json;
use std::sync::Arc;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn init_with_source(repo: &str) {
    provenance()
        .args([
            "init",
            "--path",
            repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "sources",
            "create",
            "--repo",
            repo,
            "--id",
            "source_live_inventory",
            "--name",
            "Live inventory",
        ])
        .assert()
        .success();
}

struct EmptyCheckPort;

impl provenance_porcelain::check::CheckPort for EmptyCheckPort {
    fn run<'a>(
        &'a self,
        category: provenance_porcelain::check::Category,
        _: Option<&'a str>,
    ) -> provenance_porcelain::check::PortFuture<'a> {
        Box::pin(async move {
            Ok(provenance_porcelain::check::CategoryRun::new(
                category,
                Vec::new(),
            ))
        })
    }
}

#[tokio::test]
async fn cli_uses_the_names_from_the_live_mcp_inventory() {
    use rmcp::{model::CallToolRequestParams, ServiceExt as _};

    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    init_with_source(&repo);
    let access = provenance_transport::LocalAccess::new(
        directory.path(),
        "native",
        "default",
        &"0".repeat(64),
        "127.0.0.1:1".parse().unwrap(),
    )
    .unwrap();
    let host = provenance_transport::StatementHost::with_access(Arc::new(access))
        .with_check_port(Arc::new(EmptyCheckPort));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    let get_name = tools
        .iter()
        .find(|tool| tool.input_schema.get("required") == Some(&json!(["target"])))
        .unwrap()
        .name
        .to_string();
    let check_name = tools
        .iter()
        .find(|tool| {
            tool.input_schema
                .get("properties")
                .and_then(|properties| properties.get("categories"))
                .is_some()
        })
        .unwrap()
        .name
        .to_string();

    let cli_get = provenance()
        .args([
            "source_live_inventory",
            &get_name,
            "--repo",
            &repo,
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(cli_get.status.success());
    let cli_get: serde_json::Value = serde_json::from_slice(&cli_get.stdout).unwrap();
    let mcp_get = client
        .call_tool(
            CallToolRequestParams::new(get_name.clone()).with_arguments(
                json!({"target":"source_live_inventory"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap()
        .structured_content
        .unwrap();
    for field in ["record", "view", "related", "detail", "bounds"] {
        assert_eq!(cli_get[field], mcp_get[field], "get field {field}");
    }

    let cli_check = provenance()
        .args([&check_name, "--repo", &repo, "--graph", "--format", "json"])
        .output()
        .unwrap();
    assert!(cli_check.status.success());
    let cli_check: serde_json::Value = serde_json::from_slice(&cli_check.stdout).unwrap();
    let mcp_check = client
        .call_tool(
            CallToolRequestParams::new(check_name)
                .with_arguments(json!({"categories":["graph"]}).as_object().unwrap().clone()),
        )
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(cli_check["categories"], mcp_check["categories"]);
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[test]
fn cli_and_mcp_get_bindings_translate_to_the_same_semantics() {
    let cli = provenance_cli::porcelain::parse_get(&[
        "req_alpha",
        "get",
        "--view",
        "children",
        "--depth",
        "2",
        "--kind",
        "rule",
        "--limit",
        "25",
    ])
    .unwrap();
    let mcp: provenance_transport::porcelain::GetArguments = serde_json::from_value(json!({
        "target": "req_alpha",
        "view": "children",
        "max_depth": 2,
        "returned_kinds": ["rule"],
        "limit": 25
    }))
    .unwrap();

    assert_eq!(cli, mcp.into_get_input());
}

#[test]
#[verifies("rule_porcelain_check_selector_union", examples)]
fn live_cli_and_mcp_check_selectors_keep_shared_semantics() {
    let cli = provenance_cli::porcelain::check_input_from_selectors(true, false, true);
    let mcp: provenance_transport::porcelain::CheckArguments = serde_json::from_value(json!({
        "categories": ["graph", "bindings"]
    }))
    .unwrap();

    assert_eq!(cli.categories(), mcp.into_check_input().categories());
}

#[test]
#[verifies("rule_porcelain_cli_readable_json", examples)]
fn readable_get_warns_when_the_record_or_view_is_stale() {
    let record = serde_json::from_value(json!({
        "node_type": "requirement", "schema_version": 2, "scope_id": "default",
        "id": "req_stale", "statement": "Keep the graph typed.", "status": "active"
    }))
    .unwrap();
    let outcome = provenance_porcelain::get::GetOutcome {
        record,
        result: provenance_porcelain::get::ViewResult::Children(
            provenance_porcelain::get::Traversal {
                records: Vec::new(),
                bounds: provenance_porcelain::get::Bounds {
                    limit: 50,
                    max_depth: Some(1),
                    has_more: false,
                    continuation: None,
                    truncated: false,
                },
                response_metadata: Some(provenance_core::protocol::ResponseMeta {
                    freshness_error: Some("view catch-up failed".into()),
                    ..Default::default()
                }),
            },
        ),
        record_metadata: Some(provenance_core::protocol::ResponseMeta {
            freshness_error: Some("record catch-up failed".into()),
            ..Default::default()
        }),
    };

    let readable = provenance_cli::porcelain::render_get_readable(&outcome).unwrap();

    assert!(readable.contains("warning: record freshness: record catch-up failed"));
    assert!(readable.contains("warning: view freshness: view catch-up failed"));
}

#[test]
#[verifies("rule_porcelain_cli_readable_json", examples)]
fn cli_readable_get_includes_the_record_view_context_and_bounds() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "requirements",
            "create",
            "--repo",
            &repo,
            "--id",
            "req_readable",
            "--statement",
            "Readable output identifies the selected record.",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "rules",
            "create",
            "--repo",
            &repo,
            "--id",
            "rule_readable",
            "--requirement-id",
            "req_readable",
            "--statement",
            "Show related records in readable output.",
            "--severity",
            "high",
        ])
        .assert()
        .success();

    let output = provenance()
        .args([
            "req_readable",
            "get",
            "--repo",
            &repo,
            "--view",
            "children",
            "--limit",
            "7",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let readable = String::from_utf8(output.stdout).unwrap();
    assert!(readable.contains("Readable output identifies the selected record."));
    assert!(readable.contains("view: children"));
    assert!(readable.contains("rule_readable"));
    assert!(readable.contains("bounds:"));
    assert!(readable.contains("limit=7"));
}
