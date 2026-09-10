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
fn operation_names_are_explicit() {
    let (_, mcp) = provenance_codegen::documents();
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
            "add-requirement-depends-on",
            "add-requirement-supersedes",
            "add-resolution-requirement",
            "add-resolution-supersedes",
            "add-rule-requirement",
            "add-rule-resolution",
            "add-source-reference",
            "add-source-supersedes",
            "answer-question",
            "apply",
            "begin-verification",
            "check-statement",
            "claim-question",
            "claim-topic",
            "clear-question-contradicts",
            "clear-requirement-depends-on",
            "clear-requirement-refines",
            "clear-requirement-spawned-by",
            "clear-requirement-supersedes",
            "clear-resolution-requirement",
            "clear-resolution-supersedes",
            "clear-rule-requirement",
            "clear-rule-resolution",
            "clear-source-reference",
            "clear-source-supersedes",
            "close-topic",
            "complete-verification",
            "create-assertion",
            "create-boundary",
            "create-contribution",
            "create-disposition",
            "create-domain",
            "create-proposal",
            "create-question",
            "create-requirement",
            "create-resolution",
            "create-rule",
            "create-source",
            "create-synthesis-packet",
            "create-topic",
            "evidence",
            "get",
            "impact",
            "info",
            "list-assertions",
            "list-dispositions",
            "list-messages",
            "list-proposals",
            "list-threads",
            "neighbors",
            "plan",
            "post-thread-message",
            "read-document",
            "release-question",
            "release-topic",
            "resolve-symbol",
            "search",
            "set-question-contradicts",
            "set-requirement-refines",
            "set-requirement-spawned-by",
            "stale",
            "trace",
            "update-boundary",
            "update-domain",
            "update-question",
            "update-requirement",
            "update-resolution",
            "update-rule",
            "update-source",
            "update-topic",
            "upsert-contribution",
            "upsert-synthesis-packet",
            "verification-bindings",
            "verification-runs",
        ]
    );
}

#[test]
fn operation_failure_statuses_and_repository_context_are_explicit() {
    let (document, _) = provenance_codegen::documents();
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
fn document_read_has_a_scoped_cursor_request() {
    let (document, mcp) = provenance_codegen::documents();
    let version = provenance_core::SDK_PROTOCOL_VERSION;
    let route = &document["paths"][format!("/v{version}/operations/read-document")]["post"];
    assert_eq!(route["operationId"], "readDocument");
    assert_eq!(route["x-operation-mutates"], false);
    assert_eq!(
        route["responses"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["200", "400", "401", "403", "404", "409", "500", "503"]
    );
    assert_eq!(
        route["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/ReadDocumentSuccessOutput"
    );
    let tool = mcp["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "read-document")
        .unwrap();
    let input = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(&tool["inputSchema"])
        .unwrap();
    let request = json!({"protocol_version":version,"call":{
        "context":{"repository":"selected","scope":"default"},"request":{"id":"req_shared"}
    }});
    assert!(input.is_valid(&request));
    for pointer in [
        "/call/context/repository",
        "/call/context/scope",
        "/call/request/id",
    ] {
        let mut missing = request.clone();
        let (parent, field) = pointer.rsplit_once('/').unwrap();
        missing
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(!input.is_valid(&missing), "{pointer}");
    }
    let mut paged = request;
    for limit in [1, 50, 200] {
        paged["call"]["request"]["limit"] = json!(limit);
        assert!(input.is_valid(&paged));
    }
    for limit in [0, 201] {
        paged["call"]["request"]["limit"] = json!(limit);
        assert!(!input.is_valid(&paged));
    }
    paged["call"]["request"]["limit"] = json!(1);
    paged["call"]["request"]["cursor"] = json!("opaque-continuation");
    assert!(input.is_valid(&paged));
    paged["call"]["request"]["cursor"] = json!(17);
    assert!(!input.is_valid(&paged));
    paged["call"]["request"]["cursor"] = json!(null);
    assert!(input.is_valid(&paged));
    paged["call"]["request"]["offset"] = json!(1);
    assert!(!input.is_valid(&paged));
}

#[test]
fn document_read_has_a_stamped_page_output() {
    let (document, mcp) = provenance_codegen::documents();
    let tool = mcp["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "read-document")
        .unwrap();
    let output = &document["components"]["schemas"]["ReadDocumentSuccessOutput"];
    assert_eq!(output["properties"]["operation"]["const"], "read-document");
    for field in [
        "root_id",
        "stamp",
        "protocol_version",
        "operation",
        "limit",
        "has_more",
        "next_cursor",
        "entries",
    ] {
        assert!(
            output["required"]
                .as_array()
                .unwrap()
                .contains(&json!(field)),
            "{field}"
        );
        assert!(
            tool["outputSchema"]["required"]
                .as_array()
                .unwrap()
                .contains(&json!(field)),
            "{field}"
        );
    }
    for family in [
        "requirements",
        "resolutions",
        "rules",
        "sources",
        "topics",
        "questions",
        "threads",
        "messages",
    ] {
        assert!(output["properties"].get(family).is_none(), "{family}");
    }
    assert_eq!(output["properties"]["entries"]["type"], "array");
    assert_eq!(
        output["properties"]["entries"]["items"]["$ref"],
        "#/components/schemas/ReadDocumentSuccessOutputDocumentEntry"
    );
    for entry in [
        &document["components"]["schemas"]["ReadDocumentSuccessOutputDocumentEntry"],
        &tool["outputSchema"]["$defs"]["DocumentEntry"],
    ] {
        let variants = entry["oneOf"].as_array().unwrap();
        assert_eq!(variants.len(), 4);
        for (variant, (kind, payload)) in variants.iter().zip([
            ("member", "node"),
            ("reference", "node"),
            ("thread", "thread"),
            ("message", "message"),
        ]) {
            assert_eq!(variant["properties"]["kind"]["const"], kind);
            assert_eq!(variant["required"], json!(["kind", payload]));
        }
    }
}

#[test]
fn mcp_list_outputs_wrap_the_complete_http_array() {
    let (document, mcp) = provenance_codegen::documents();
    for name in [
        "verification-bindings",
        "verification-runs",
        "list-threads",
        "list-messages",
        "list-proposals",
        "list-dispositions",
        "list-assertions",
    ] {
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
            let expected = [
                "create-contribution",
                "upsert-contribution",
                "create-synthesis-packet",
                "upsert-synthesis-packet",
                "set-requirement-refines",
                "clear-requirement-refines",
                "add-requirement-depends-on",
                "clear-requirement-depends-on",
                "add-requirement-supersedes",
                "clear-requirement-supersedes",
                "set-requirement-spawned-by",
                "clear-requirement-spawned-by",
                "add-rule-requirement",
                "clear-rule-requirement",
                "add-rule-resolution",
                "clear-rule-resolution",
                "add-resolution-requirement",
                "clear-resolution-requirement",
                "add-resolution-supersedes",
                "clear-resolution-supersedes",
                "add-source-supersedes",
                "clear-source-supersedes",
                "set-question-contradicts",
                "clear-question-contradicts",
                "clear-source-reference",
                "claim-topic",
                "release-topic",
                "close-topic",
                "claim-question",
                "release-question",
                "answer-question",
                "update-source",
                "update-resolution",
                "update-requirement",
                "update-rule",
                "update-domain",
                "update-boundary",
                "update-topic",
                "update-question",
                "create-domain",
                "create-boundary",
                "create-topic",
                "create-question",
                "apply",
                "begin-verification",
                "complete-verification",
                "create-source",
                "create-requirement",
                "create-resolution",
                "create-rule",
                "add-source-reference",
                "post-thread-message",
                "create-proposal",
                "create-assertion",
                "create-disposition",
            ]
            .iter()
            .any(|name| path.ends_with(&format!("/{name}")));
            assert_eq!(operation["x-operation-mutates"], expected, "{path}");
        }
    }
}
