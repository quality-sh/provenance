use provenance_macros::verifies;
use provenance_porcelain::{Action, Outcome, RecordRequest};
use serde_json::json;

#[test]
#[verifies("rule_porcelain_mcp_target_argument", examples)]
fn mcp_get_translates_its_structured_target_argument() {
    let arguments: provenance_transport::porcelain::GetArguments =
        serde_json::from_value(json!({"target": "req_alpha"})).unwrap();

    assert_eq!(
        arguments.into_request(),
        RecordRequest::new("req_alpha", Action::Get)
    );
}

#[test]
#[verifies("rule_porcelain_mcp_readable_structured", examples)]
fn mcp_result_contains_readable_and_structured_content() {
    let data = json!({"id": "req_alpha", "kind": "requirement"});
    let outcome = Outcome::new(
        "Requirement req_alpha: Keep the surfaces consistent.",
        data.clone(),
    );

    let result = provenance_transport::porcelain::render(outcome).unwrap();

    assert_eq!(
        result.content[0].as_text().unwrap().text,
        "Requirement req_alpha: Keep the surfaces consistent."
    );
    assert_eq!(result.structured_content, Some(data));
}
