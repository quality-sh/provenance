use assert_cmd::Command;
use provenance_macros::verifies;
use provenance_mcp::Client;
use provenance_porcelain::check::{
    BindingContext, BindingPolicy, Category, CategoryRun, CheckInput, GapsContext, PortFuture,
    Refusal,
};
use serde_json::json;
use std::sync::Arc;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn init_with_source(repo: &str) {
    let init = "init --scope default --path-prefix . --path";
    provenance()
        .args(init.split(' '))
        .arg(repo)
        .assert()
        .success();
    let create = "source_live_inventory create --type source --name";
    let mut cli = provenance();
    cli.args(create.split(' ')).arg("Live inventory");
    cli.args(["--repo", repo]).assert().success();
}

/// The MCP host that the CLI builds for one local repository.
fn local_host(path: &std::path::Path) -> provenance_transport::StatementHost {
    let repository = provenance_store::repository::Repository::open(path, "default").unwrap();
    let access = provenance_store::operations::LocalRepository::bind(repository, "native");
    provenance_transport::StatementHost::with_access(Arc::new(access.unwrap()))
}

async fn assert_get_readable(client: &Client, repo: &str, name: &str) {
    let cli = provenance()
        .args(["source_live_inventory", name, "--repo", repo])
        .output()
        .unwrap();
    assert!(cli.status.success());
    let mcp = client
        .call(name, json!({"target":"source_live_inventory"}))
        .await;
    assert_eq!(
        String::from_utf8(cli.stdout).unwrap().trim_end(),
        mcp["content"][0]["text"]
    );
}

async fn assert_check_readable(client: &Client, repo: &str, name: &str) {
    let cli = provenance()
        .args([name, "--repo", repo, "--graph"])
        .output()
        .unwrap();
    assert!(cli.status.success());
    let mcp = client.call(name, json!({"categories":["graph"]})).await;
    assert_eq!(
        String::from_utf8(cli.stdout).unwrap().trim_end(),
        mcp["content"][0]["text"]
    );
}

struct EmptyCheckPort;

impl provenance_porcelain::check::CheckPort for EmptyCheckPort {
    fn run<'a>(&'a self, category: Category, _: Option<&'a str>) -> PortFuture<'a> {
        Box::pin(async move {
            Ok(match category {
                Category::Graph => CategoryRun::Graph {
                    findings: Vec::new(),
                    refusal: Refusal::None,
                },
                Category::Statements => CategoryRun::Statements {
                    findings: Vec::new(),
                    context: None,
                    refusal: Refusal::None,
                },
                Category::Bindings => CategoryRun::Bindings {
                    findings: Vec::new(),
                    context: BindingContext {
                        policy: BindingPolicy::Warning,
                    },
                    refusal: Refusal::None,
                },
                Category::Gaps => CategoryRun::Gaps {
                    findings: Vec::new(),
                    context: GapsContext { scopes: Vec::new() },
                },
            })
        })
    }
}

#[tokio::test]
async fn cli_uses_the_names_from_the_live_mcp_inventory() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    init_with_source(&repo);
    let host = local_host(directory.path()).with_check_port(Arc::new(EmptyCheckPort));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = Client::connect(client_io).await.unwrap();
    let tools = client.tools().await;
    let get_name = tools
        .iter()
        .find(|tool| tool["inputSchema"]["required"] == json!(["target"]))
        .unwrap()["name"]
        .as_str()
        .unwrap()
        .to_owned();
    let get_tool = tools.iter().find(|tool| tool["name"] == get_name).unwrap();
    let get_schema = get_tool["outputSchema"].clone();
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
            tool["inputSchema"]["properties"]
                .get("categories")
                .is_some()
        })
        .unwrap()["name"]
        .as_str()
        .unwrap()
        .to_owned();

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
        .call(&get_name, json!({"target":"source_live_inventory"}))
        .await["structuredContent"]
        .clone();
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
        .call(&check_name, json!({"categories":["graph"]}))
        .await["structuredContent"]
        .clone();
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
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    init_with_source(&repo);
    let host = local_host(directory.path());
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = Client::connect(client_io).await.unwrap();
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
        let mcp = client.call("get", arguments.clone()).await;
        assert_eq!(mcp["isError"], true, "{arguments}");
        assert_eq!(mcp["structuredContent"]["error"], cli, "{arguments}");
    }
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

/// Runs the CLI on `repo` with the arguments in `command`, split on spaces, then `extra`.
fn cli(repo: &str, command: &str, extra: &[&str]) -> std::process::Output {
    let mut cli = provenance();
    cli.args(command.split(' '))
        .args(extra)
        .args(["--repo", repo]);
    cli.output().unwrap()
}

/// A repository where `code.rs` binds `rule_pay` to `pay` and `rule_other` to `other`.
fn init_with_bound_code(repo: &str) {
    init_with_source(repo);
    let statement = ["--statement", "Shared needle requirement."];
    let created = cli(repo, "req_pay create --type requirement", &statement);
    assert!(created.status.success());
    for (id, statement, status) in [
        ("rule_pay", "Pay overtime after the threshold.", "active"),
        ("rule_other", "Other pay rule.", "deprecated"),
    ] {
        let command = format!(
            "{id} create --type rule --requirement-id req_pay --severity medium --status {status}"
        );
        let created = cli(repo, &command, &["--statement", statement]);
        assert!(created.status.success());
    }
    let code = "#[rule(\"rule_other\")]\nfn other() {}\n#[rule(\"rule_pay\")]\nfn pay() {}\n#[verifies(\"rule_pay\", examples)]\nfn checks_pay() {}\n";
    std::fs::write(std::path::Path::new(repo).join("code.rs"), code).unwrap();
}

#[tokio::test]
#[verifies("rule_search_file_same_on_cli_and_mcp", examples)]
async fn cli_and_mcp_file_search_return_the_same_result() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    init_with_bound_code(&repo);
    let host = local_host(directory.path());
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = Client::connect(client_io).await.unwrap();
    let tools = client.tools().await;
    let search = tools.iter().find(|tool| tool["name"] == "search").unwrap();
    for field in ["file", "symbol", "line"] {
        assert!(search["inputSchema"]["properties"][field].is_object());
    }
    // The search schemas are generated as JSON Schema 2020-12 and sent without `$schema`.
    let output = search["outputSchema"].clone();
    let validator = jsonschema::draft202012::new(&output).unwrap();
    let call = |arguments: serde_json::Value| client.call("search", arguments);

    let lookup = "search --file code.rs --symbol pay";
    let cli_json = cli(&repo, lookup, &["--format", "json"]);
    assert!(cli_json.status.success());
    let cli_json: serde_json::Value = serde_json::from_slice(&cli_json.stdout).unwrap();
    let cli_text = cli(&repo, lookup, &[]);
    let mcp = call(json!({"file":"code.rs","symbol":"pay"})).await;
    assert_ne!(mcp["isError"], true, "{mcp}");
    let structured = &mcp["structuredContent"];
    assert_eq!(&cli_json, structured);
    assert_eq!(cli_json["nodes"][0]["id"], "rule_pay");
    assert!(validator.is_valid(structured), "{structured}");
    let cli_text = String::from_utf8(cli_text.stdout).unwrap();
    assert_eq!(cli_text.trim_end(), mcp["content"][0]["text"]);

    let cli_refusal = cli(&repo, "search --symbol pay", &[]);
    assert!(!cli_refusal.status.success());
    let mcp_refusal = call(json!({"symbol":"pay"})).await;
    assert_eq!(mcp_refusal["isError"], true, "{mcp_refusal}");
    let error = &mcp_refusal["structuredContent"]["error"];
    let message = error["message"].as_str().unwrap();
    assert!(message.contains("search --file <path>"), "{message}");
    let cli_message = String::from_utf8(cli_refusal.stderr).unwrap();
    assert_eq!(cli_message.trim_end(), format!("Error: {message}"));
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[test]
#[verifies("rule_porcelain_check_selector_union", examples)]
fn live_cli_and_mcp_check_selectors_keep_shared_semantics() {
    let cli = provenance_cli::porcelain::check_input_from_selectors(true, false, true, None);
    let mcp: CheckInput = serde_json::from_value(json!({
        "categories": ["graph", "bindings"]
    }))
    .unwrap();

    assert_eq!(cli.categories(), mcp.categories());
}

#[test]
#[verifies("rule_porcelain_check_statement_field_shared", examples)]
fn live_cli_and_mcp_check_statement_fields_produce_the_same_input() {
    let statement = "The pump stops.";
    let cli = provenance_cli::porcelain::check_input_from_selectors(
        false,
        false,
        false,
        Some(statement.to_owned()),
    );
    let mcp: CheckInput = serde_json::from_value(json!({"statement": statement})).unwrap();

    assert_eq!(cli, mcp);
    assert_eq!(cli.categories(), [Category::Statements]);
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
            "req_readable",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "Readable output identifies the selected record.",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "rule_readable",
            "create",
            "--type",
            "rule",
            "--repo",
            &repo,
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
