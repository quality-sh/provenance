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
fn sixteen_operation_names_and_failure_statuses_are_explicit() {
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
            "apply",
            "begin-verification",
            "check-statement",
            "complete-verification",
            "evidence",
            "get",
            "impact",
            "info",
            "neighbors",
            "plan",
            "resolve-symbol",
            "search",
            "stale",
            "trace",
            "verification-bindings",
            "verification-runs"
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

#[test]
fn mcp_list_outputs_wrap_the_complete_http_array() {
    let (document, mcp) = provenance_codegen::documents();
    for name in ["verification-bindings", "verification-runs"] {
        let tool = mcp["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap();
        let schema = &tool["outputSchema"];
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["result"]["type"], "array");
        let validator = jsonschema::JSONSchema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .compile(schema)
            .unwrap();
        assert!(validator.is_valid(&json!({"result":[]})));
        assert!(!validator.is_valid(&json!([])));
        let path = format!(
            "/v{}/operations/{name}",
            provenance_core::protocol::SDK_PROTOCOL_VERSION
        );
        let reference = document["paths"][path]["post"]["responses"]["200"]["content"]
            ["application/json"]["schema"]["$ref"]
            .as_str()
            .unwrap();
        assert_eq!(
            document
                .pointer(reference.strip_prefix('#').unwrap())
                .unwrap()["type"],
            "array"
        );
    }
}

#[test]
fn mutation_classification_comes_from_the_catalog() {
    let (document, _) = provenance_codegen::documents();
    for (path, route) in document["paths"].as_object().unwrap() {
        if let Some(operation) = route.get("post") {
            let expected = ["apply", "begin-verification", "complete-verification"]
                .iter()
                .any(|name| path.ends_with(&format!("/{name}")));
            assert_eq!(operation["x-operation-mutates"], expected, "{path}");
        }
    }
}
