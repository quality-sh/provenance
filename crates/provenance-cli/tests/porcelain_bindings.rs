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

async fn assert_get_readable(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    repo: &str,
    name: &str,
) {
    use rmcp::model::CallToolRequestParams;

    let cli = provenance()
        .args(["source_live_inventory", name, "--repo", repo])
        .output()
        .unwrap();
    assert!(cli.status.success());
    let mcp = client
        .call_tool(
            CallToolRequestParams::new(name.to_owned()).with_arguments(
                json!({"target":"source_live_inventory"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        String::from_utf8(cli.stdout).unwrap().trim_end(),
        mcp.content[0].as_text().unwrap().text
    );
}

async fn assert_check_readable(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    repo: &str,
    name: &str,
) {
    use rmcp::model::CallToolRequestParams;

    let cli = provenance()
        .args([name, "--repo", repo, "--graph"])
        .output()
        .unwrap();
    assert!(cli.status.success());
    let mcp = client
        .call_tool(
            CallToolRequestParams::new(name.to_owned())
                .with_arguments(json!({"categories":["graph"]}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    assert_eq!(
        String::from_utf8(cli.stdout).unwrap().trim_end(),
        mcp.content[0].as_text().unwrap().text
    );
}

struct EmptyCheckPort;

impl provenance_porcelain::check::CheckPort for EmptyCheckPort {
    fn run<'a>(
        &'a self,
        category: provenance_porcelain::check::Category,
        _: Option<&'a str>,
    ) -> provenance_porcelain::check::PortFuture<'a> {
        Box::pin(async move {
            Ok(match category {
                provenance_porcelain::check::Category::Graph => {
                    provenance_porcelain::check::CategoryRun::Graph {
                        findings: Vec::new(),
                        refusal: provenance_porcelain::check::Refusal::None,
                    }
                }
                provenance_porcelain::check::Category::Statements => {
                    provenance_porcelain::check::CategoryRun::Statements {
                        findings: Vec::new(),
                        context: None,
                        refusal: provenance_porcelain::check::Refusal::None,
                    }
                }
                provenance_porcelain::check::Category::Bindings => {
                    provenance_porcelain::check::CategoryRun::Bindings {
                        findings: Vec::new(),
                        context: provenance_porcelain::check::BindingContext {
                            policy: provenance_porcelain::check::BindingPolicy::Warning,
                        },
                        refusal: provenance_porcelain::check::Refusal::None,
                    }
                }
            })
        })
    }
}

#[tokio::test]
async fn cli_uses_the_names_from_the_live_mcp_inventory() {
    use rmcp::{model::CallToolRequestParams, ServiceExt as _};

    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    init_with_source(&repo);
    let access = provenance_store::operations::LocalRepository::bind(
        provenance_store::repository::Repository::open(directory.path(), "default").unwrap(),
        "native",
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
    let get_tool = tools.iter().find(|tool| tool.name == get_name).unwrap();
    let get_schema = json!(get_tool.output_schema.as_ref().unwrap());
    let get_validator = compile(&get_schema).unwrap();
    let invalid_record = json!({
        "record": {"id":"source_live_inventory", "kind":"source", "value":42},
        "view":"record", "related":[], "detail":null, "bounds":null
    });
    assert!(
        !get_validator.is_valid(&invalid_record),
        "record payload must be typed"
    );
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
    assert_get_readable(&client, &repo, &get_name).await;

    let cli_check = provenance()
        .args([&check_name, "--repo", &repo, "--graph", "--format", "json"])
        .output()
        .unwrap();
    assert!(cli_check.status.success());
    let cli_check: serde_json::Value = serde_json::from_slice(&cli_check.stdout).unwrap();
    let mcp_check = client
        .call_tool(
            CallToolRequestParams::new(check_name.clone())
                .with_arguments(json!({"categories":["graph"]}).as_object().unwrap().clone()),
        )
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(cli_check["categories"], mcp_check["categories"]);
    assert_check_readable(&client, &repo, &check_name).await;
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

fn cli_get_error(repo: &str, args: &[&str]) -> serde_json::Value {
    let output = provenance()
        .args(args)
        .args(["--repo", repo, "--format", "json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let refusal: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    refusal["error"].clone()
}

#[tokio::test]
#[verifies("rule_mcp_get_failure_matches_cli", examples)]
async fn mcp_get_failures_are_the_cli_json_refusals() {
    use rmcp::{model::CallToolRequestParams, ServiceExt as _};

    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    init_with_source(&repo);
    let access = provenance_store::operations::LocalRepository::bind(
        provenance_store::repository::Repository::open(directory.path(), "default").unwrap(),
        "native",
    )
    .unwrap();
    let host = provenance_transport::StatementHost::with_access(Arc::new(access));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let limit = json!({"target":"source_live_inventory","limit":5});
    let depth = json!({"target":"source_live_inventory","view":"children","max_depth":0});
    let unknown = json!({"target":"source_live_inventory","unknown":true});

    for (cli, arguments) in [
        ("source_missing get", json!({"target":"source_missing"})),
        ("source_live_inventory get --limit 5", limit),
        ("source_live_inventory get --view children --depth 0", depth),
        ("source_live_inventory get --limit 5", unknown),
    ] {
        let cli = cli_get_error(&repo, &cli.split(' ').collect::<Vec<_>>());
        let mcp = client
            .call_tool(
                CallToolRequestParams::new("get")
                    .with_arguments(arguments.as_object().unwrap().clone()),
            )
            .await
            .unwrap();
        assert_eq!(mcp.is_error, Some(true), "{arguments}");
        assert_eq!(mcp.structured_content.unwrap()["error"], cli, "{arguments}");
    }
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

fn create(repo: &str, args: &[&str]) {
    provenance()
        .args(&args[..2])
        .args(["--repo", repo])
        .args(&args[2..])
        .assert()
        .success();
}

/// A repository with two Rules bound in `code.rs`: one to `pay`, one to `other`.
fn init_with_bound_code(repo: &str) {
    init_with_source(repo);
    create(
        repo,
        &[
            "requirements",
            "create",
            "--id",
            "req_pay",
            "--statement",
            "Shared needle requirement.",
        ],
    );
    for (id, statement, status) in [
        ("rule_pay", "Pay overtime after the threshold.", "active"),
        ("rule_other", "Other pay rule.", "deprecated"),
    ] {
        create(
            repo,
            &[
                "rules",
                "create",
                "--id",
                id,
                "--requirement-id",
                "req_pay",
                "--statement",
                statement,
                "--status",
                status,
                "--severity",
                "medium",
            ],
        );
    }
    std::fs::write(
        std::path::Path::new(repo).join("code.rs"),
        concat!(
            "#[rule(\"rule_other\")]\n",
            "fn other() {}\n",
            "#[rule(\"rule_pay\")]\n",
            "fn pay() {}\n",
            "#[verifies(\"rule_pay\", examples)]\n",
            "fn checks_pay() {}\n",
        ),
    )
    .unwrap();
}

#[tokio::test]
#[verifies("rule_search_file_same_on_cli_and_mcp", examples)]
async fn cli_and_mcp_file_search_return_the_same_result() {
    use rmcp::{model::CallToolRequestParams, ServiceExt as _};

    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    init_with_bound_code(&repo);
    let access = provenance_store::operations::LocalRepository::bind(
        provenance_store::repository::Repository::open(directory.path(), "default").unwrap(),
        "native",
    )
    .unwrap();
    let host = provenance_transport::StatementHost::with_access(Arc::new(access));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    let search = tools.iter().find(|tool| tool.name == "search").unwrap();
    for field in ["file", "symbol", "line"] {
        assert!(
            search.input_schema["properties"][field].is_object(),
            "{field}"
        );
    }
    let mut output = json!(search.output_schema.as_ref().unwrap());
    let success = output["oneOf"][0].take();
    let output = output.as_object_mut().unwrap();
    output.remove("oneOf");
    output.extend(success.as_object().unwrap().clone());
    let validator = compile(&serde_json::Value::Object(output.clone())).unwrap();

    let search_args = ["search", "--file", "code.rs", "--symbol", "pay", "--repo"];
    let cli_json = provenance()
        .args(search_args)
        .args([repo.as_str(), "--format", "json"])
        .output()
        .unwrap();
    assert!(
        cli_json.status.success(),
        "{}",
        String::from_utf8_lossy(&cli_json.stderr)
    );
    let cli_json: serde_json::Value = serde_json::from_slice(&cli_json.stdout).unwrap();
    let cli_text = provenance().args(search_args).arg(&repo).output().unwrap();
    assert!(cli_text.status.success());
    let mcp = client
        .call_tool(
            CallToolRequestParams::new("search").with_arguments(
                json!({"file":"code.rs","symbol":"pay"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_ne!(mcp.is_error, Some(true), "{mcp:?}");
    let structured = mcp.structured_content.as_ref().unwrap();
    assert_eq!(&cli_json, structured);
    assert_eq!(cli_json["nodes"][0]["id"], "rule_pay");
    let errors = validator
        .iter_errors(structured)
        .map(|error| format!("{} at {}", error, error.instance_path()))
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        String::from_utf8(cli_text.stdout).unwrap().trim_end(),
        mcp.content[0].as_text().unwrap().text
    );

    let cli_refusal = provenance()
        .args(["search", "--symbol", "pay", "--repo", &repo])
        .output()
        .unwrap();
    assert!(!cli_refusal.status.success());
    let mcp_refusal = client
        .call_tool(
            CallToolRequestParams::new("search")
                .with_arguments(json!({"symbol":"pay"}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    assert_eq!(mcp_refusal.is_error, Some(true), "{mcp_refusal:?}");
    let message = mcp_refusal.structured_content.as_ref().unwrap()["error"]["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        message.contains("provenance search --file <path>"),
        "{message}"
    );
    assert_eq!(
        String::from_utf8(cli_refusal.stderr).unwrap().trim_end(),
        format!("Error: {message}")
    );
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[test]
#[verifies("rule_porcelain_check_selector_union", examples)]
fn live_cli_and_mcp_check_selectors_keep_shared_semantics() {
    let cli = provenance_cli::porcelain::check_input_from_selectors(true, false, true);
    let mcp: provenance_porcelain::check::CheckInput = serde_json::from_value(json!({
        "categories": ["graph", "bindings"]
    }))
    .unwrap();

    assert_eq!(cli.categories(), mcp.categories());
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
                shaping: None,
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
            "--status",
            "active",
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

/// Read a schema that has no `$schema` keyword as a Draft 7 document.
fn compile(
    schema: &serde_json::Value,
) -> Result<jsonschema::Validator, jsonschema::ValidationError<'static>> {
    if schema.get("$schema").is_some() {
        jsonschema::validator_for(schema)
    } else {
        jsonschema::draft7::new(schema)
    }
}
