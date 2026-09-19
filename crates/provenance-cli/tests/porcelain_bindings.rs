use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt as _;
use provenance_macros::verifies;
use serde_json::json;
use std::sync::Arc;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

struct EmptyCheckPort;

impl provenance_porcelain::check::CheckPort for EmptyCheckPort {
    fn run<'a>(
        &'a self,
        _: provenance_porcelain::check::Category,
        _: Option<&'a str>,
    ) -> provenance_porcelain::check::PortFuture<'a> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[tokio::test]
#[verifies("rule_porcelain_action_names_match", conformance)]
async fn cli_uses_the_names_from_the_live_mcp_inventory() {
    use rmcp::ServiceExt as _;

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
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "source_live_inventory",
            "--name",
            "Live inventory",
        ])
        .assert()
        .success();
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
        .find(|tool| tool.input_schema["required"] == json!(["target"]))
        .unwrap()
        .name
        .to_string();
    let check_name = tools
        .iter()
        .find(|tool| tool.input_schema["properties"].get("categories").is_some())
        .unwrap()
        .name
        .to_string();

    provenance()
        .args([
            "source_live_inventory",
            &get_name,
            "--repo",
            &repo,
            "--format",
            "json",
        ])
        .assert()
        .success();
    provenance()
        .args([&check_name, "--repo", &repo, "--graph", "--format", "json"])
        .assert()
        .success();
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[test]
fn bare_help_flag_runs_before_a_repository_exists() {
    let directory = tempfile::tempdir().unwrap();

    provenance()
        .current_dir(directory.path())
        .arg("--help")
        .assert()
        .success();
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
#[verifies("rule_porcelain_action_names_match", conformance)]
#[verifies("rule_porcelain_cli_target_action_order", examples)]
#[verifies("rule_porcelain_cli_readable_json", examples)]
fn target_first_get_runs_through_the_cli_entrypoint() {
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
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "source_porcelain_entrypoint",
            "--name",
            "Porcelain entrypoint",
        ])
        .assert()
        .success();

    let output = provenance()
        .args([
            "source_porcelain_entrypoint",
            "get",
            "--repo",
            &repo,
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["record"]["id"], "source_porcelain_entrypoint");
    assert_eq!(value["record"]["kind"], "source");
    assert_eq!(value["view"], "record");
}

#[test]
#[verifies("rule_porcelain_cli_target_action_order", examples)]
fn target_first_get_accepts_global_options_before_the_target() {
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
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "source_global_options",
            "--name",
            "Global options",
        ])
        .assert()
        .success();

    provenance()
        .args([
            "--repo",
            &repo,
            "--format",
            "json",
            "source_global_options",
            "get",
        ])
        .assert()
        .success();
}

#[test]
#[verifies("rule_porcelain_cli_target_action_order", examples)]
fn explicit_get_disambiguates_a_target_that_matches_a_legacy_command() {
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
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "sources",
            "--name",
            "Reserved target",
        ])
        .assert()
        .success();

    let output = provenance()
        .args(["sources", "get", "--repo", &repo, "--format", "json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["record"]["id"], "sources");
}

#[test]
fn a_builtin_command_takes_precedence_over_a_matching_record_id() {
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
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "check",
            "--name",
            "Command-shaped target",
        ])
        .assert()
        .success();

    let output = provenance()
        .args(["check", "--repo", &repo, "--format", "json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value["categories"].is_array(), "{value}");
}

#[test]
#[verifies("rule_porcelain_get_is_default_action", examples)]
fn a_non_command_record_id_uses_get_when_the_action_is_omitted() {
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
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "source_bare_get",
            "--name",
            "Bare get",
        ])
        .assert()
        .success();

    let output = provenance()
        .args(["source_bare_get", "--repo", &repo, "--format", "json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["record"]["id"], "source_bare_get");
}

#[test]
fn a_bare_get_lookup_error_is_returned_without_command_fallback() {
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
        .args(["missing_record", "--repo", &repo])
        .assert()
        .failure()
        .stderr(predicates::str::contains("record does not exist"))
        .stderr(predicates::str::contains("unrecognized subcommand").not());
}

#[test]
#[verifies("rule_porcelain_cli_target_action_order", examples)]
fn explicit_get_reads_a_record_that_matches_a_builtin_command() {
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
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "check",
            "--name",
            "Command-shaped target",
        ])
        .assert()
        .success();

    let output = provenance()
        .args(["check", "get", "--repo", &repo, "--format", "json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["record"]["id"], "check");
    assert_eq!(value["record"]["kind"], "source");
}

#[test]
#[verifies("rule_porcelain_cli_readable_json", examples)]
fn readable_get_warns_when_the_record_or_view_is_stale() {
    let outcome = provenance_porcelain::get::GetOutcome {
        record: provenance_porcelain::get::Record::new(
            "req_stale",
            "requirement",
            json!({"id":"req_stale"}),
        ),
        view: provenance_porcelain::get::View::Children,
        related: Vec::new(),
        detail: None,
        bounds: None,
        record_metadata: Some(json!({"freshness_error":"record catch-up failed"})),
        view_metadata: Some(json!({"freshness_error":"view catch-up failed"})),
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
