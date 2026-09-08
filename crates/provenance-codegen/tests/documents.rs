use serde_json::json;

#[test]
fn statement_documents_have_named_operation_and_closed_call_schema() {
    let (openapi, mcp) = provenance_codegen::documents();
    assert_eq!(openapi["openapi"], "3.1.0");
    let version = provenance_core::protocol::SDK_PROTOCOL_VERSION;
    let path = format!("/v{version}/operations/check-statement");
    assert_eq!(
        openapi["paths"][&path]["post"]["operationId"],
        "checkStatement"
    );
    assert_eq!(mcp["tools"][0]["name"], "check-statement");
    assert_eq!(
        mcp["tools"][0]["inputSchema"]["properties"]["protocol_version"]["const"],
        json!(version)
    );
    assert_eq!(
        mcp["tools"][0]["inputSchema"]["required"],
        json!(["protocol_version", "call"])
    );
    assert!(openapi["components"]["schemas"].as_object().unwrap().len() > 3);
}
