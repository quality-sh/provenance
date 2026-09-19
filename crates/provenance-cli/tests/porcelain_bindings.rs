use provenance_macros::verifies;
use provenance_porcelain::{Action, Outcome, RecordRequest};
use serde_json::json;

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
