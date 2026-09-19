use assert_cmd::Command;
use provenance_macros::verifies;
use provenance_porcelain::{Action, Outcome, RecordRequest};
use serde_json::json;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

#[test]
#[verifies("rule_porcelain_action_names_match", conformance)]
#[verifies("rule_porcelain_regular_graph_work_has_commands", examples)]
fn cli_and_mcp_expose_the_scoped_porcelain_actions() {
    let cli = provenance_cli::porcelain::action_names();
    let mcp = provenance_transport::porcelain::action_names();

    assert_eq!(cli, mcp);
    assert_eq!(cli, &["get", "check"]);
}

#[test]
#[verifies("rule_porcelain_cli_target_action_order", examples)]
fn cli_places_the_record_target_before_get() {
    let parsed = provenance_cli::porcelain::parse_record(&["req_alpha", "get"]).unwrap();

    assert_eq!(parsed, RecordRequest::new("req_alpha", Action::Get));
    assert!(provenance_cli::porcelain::parse_record(&["get", "req_alpha"]).is_err());
}

#[test]
#[verifies("rule_porcelain_get_is_default_action", examples)]
fn cli_defaults_a_bare_record_target_to_get() {
    let parsed = provenance_cli::porcelain::parse_record(&["req_alpha"]).unwrap();

    assert_eq!(parsed, RecordRequest::new("req_alpha", Action::Get));
}

#[test]
#[verifies("rule_porcelain_cli_readable_json", examples)]
fn cli_renders_readable_output_by_default_and_json_on_request() {
    let outcome = Outcome::new(
        "Requirement req_alpha: Keep the surfaces consistent.",
        json!({"id": "req_alpha", "kind": "requirement"}),
    );

    let readable = provenance_cli::porcelain::render(&outcome, None).unwrap();
    let json = provenance_cli::porcelain::render(
        &outcome,
        Some(provenance_cli::porcelain::OutputFormat::Json),
    )
    .unwrap();

    assert_eq!(
        readable,
        "Requirement req_alpha: Keep the surfaces consistent."
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&json).unwrap(),
        outcome.data
    );
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
