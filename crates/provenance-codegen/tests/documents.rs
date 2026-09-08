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

#[test]
fn six_operation_names_and_failure_statuses_are_explicit() {
    let (document, mcp) = provenance_codegen::documents();
    let mut names = mcp["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "check-statement",
            "get",
            "info",
            "neighbors",
            "search",
            "trace"
        ]
    );
    let paths = &document["paths"];
    let version = provenance_core::protocol::SDK_PROTOCOL_VERSION;
    assert!(
        paths[format!("/v{version}/operations/check-statement")]["post"]["responses"]
            .get("409")
            .is_none()
    );
    assert!(
        paths[format!("/v{version}/operations/get")]["post"]["responses"]
            .get("409")
            .is_some()
    );
    let context = &document["components"]["schemas"]["InfoRequestInput"]["properties"]["context"];
    let resolved = document
        .pointer(context["$ref"].as_str().unwrap().strip_prefix('#').unwrap())
        .unwrap();
    assert_eq!(resolved["required"], json!(["repository"]));
    assert!(resolved["properties"].get("scope").is_none());
}
