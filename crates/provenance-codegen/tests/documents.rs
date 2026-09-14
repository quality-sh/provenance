use serde_json::{json, Value};

fn operation<'a>(document: &'a Value, path: &str, method: &str) -> &'a Value {
    &document["paths"][path][method]
}

fn variants(schema: &Value) -> Vec<&Value> {
    schema["oneOf"]
        .as_array()
        .or_else(|| schema["anyOf"].as_array())
        .map_or_else(|| vec![schema], |variants| variants.iter().collect())
}

#[test]
fn document_exports_unversioned_resource_routes() {
    let (openapi, _) = provenance_codegen::documents();
    assert_eq!(openapi["openapi"], "3.1.0");
    let paths = openapi["paths"].as_object().unwrap();
    assert!(paths.contains_key("/metadata"));
    assert!(paths.contains_key("/sources"));
    assert!(paths.contains_key("/sources/{id}"));
    assert!(paths.contains_key("/requirements/{id}/document"));
    assert!(paths.contains_key("/verification-runs/begin-verification"));
    assert!(paths
        .keys()
        .all(|path| !path.starts_with("/v9/") && !path.contains("/operations/")));
}

#[test]
fn operation_ids_and_mutation_flags_come_from_routes() {
    let (openapi, _) = provenance_codegen::documents();
    let create = operation(&openapi, "/sources", "post");
    assert_eq!(create["operationId"], "createSource");
    assert_eq!(create["x-operation-mutates"], true);
    let list = operation(&openapi, "/sources", "get");
    assert_eq!(list["operationId"], "listSources");
    assert_eq!(list["x-operation-mutates"], false);
    assert_eq!(
        operation(&openapi, "/statement-checks", "post")["x-operation-mutates"],
        false
    );
}

#[test]
fn requests_bind_connection_and_path_identity_outside_data() {
    let (openapi, _) = provenance_codegen::documents();
    let request = operation(&openapi, "/sources/{id}", "patch")["requestBody"]["content"]
        ["application/json"]["schema"]["$ref"]
        .as_str()
        .unwrap();
    let schema = openapi.pointer(request.strip_prefix('#').unwrap()).unwrap();
    assert_eq!(schema["required"], json!(["data"]));
    let data = &schema["properties"]["data"];
    for field in ["repository", "scope", "scope_id", "id"] {
        assert!(data["properties"].get(field).is_none(), "{field}");
    }
}

#[test]
fn every_response_uses_the_shared_envelope() {
    let (openapi, mcp) = provenance_codegen::documents();
    for (path, item) in openapi["paths"].as_object().unwrap() {
        for operation in item.as_object().unwrap().values() {
            for (status, response) in operation["responses"].as_object().unwrap() {
                let reference = response["content"]["application/json"]["schema"]["$ref"]
                    .as_str()
                    .unwrap();
                let schema = openapi
                    .pointer(reference.strip_prefix('#').unwrap())
                    .unwrap();
                for schema in variants(schema) {
                    let required = schema["required"].as_array().unwrap();
                    assert!(required.contains(&json!("meta")), "{path}");
                    assert!(
                        required.contains(&json!(if status == "200" { "data" } else { "error" })),
                        "{path}"
                    );
                    assert!(
                        schema["properties"].get("protocol_version").is_none(),
                        "{path}"
                    );
                    assert!(schema["properties"].get("operation").is_none(), "{path}");
                }
            }
        }
    }
    for tool in mcp["tools"].as_array().unwrap() {
        assert!(tool["description"].as_str().unwrap().len() >= 20);
        assert_ne!(tool["description"], "Invoke the shared operation.");
        for schema in variants(&tool["outputSchema"]) {
            assert_eq!(schema["type"], "object");
            assert!(schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!("meta")));
        }
    }
}

#[test]
fn list_envelopes_put_records_in_data_items() {
    let (openapi, _) = provenance_codegen::documents();
    let reference = operation(&openapi, "/sources", "get")["responses"]["200"]["content"]
        ["application/json"]["schema"]["$ref"]
        .as_str()
        .unwrap();
    let schema = openapi
        .pointer(reference.strip_prefix('#').unwrap())
        .unwrap();
    let variants = variants(schema);
    let list = variants
        .iter()
        .find(|schema| schema["properties"]["data"]["required"] == json!(["items"]))
        .expect("list success has a data.items variant");
    for schema in [*list] {
        assert_eq!(schema["properties"]["data"]["required"], json!(["items"]));
        assert_eq!(
            schema["properties"]["data"]["properties"]["items"]["type"],
            "array"
        );
    }
}

#[test]
fn declared_statuses_include_every_runtime_failure_family() {
    let (openapi, _) = provenance_codegen::documents();
    for (path, item) in openapi["paths"].as_object().unwrap() {
        for operation in item.as_object().unwrap().values() {
            let statuses = operation["responses"].as_object().unwrap();
            for status in ["400", "401", "403", "500", "503"] {
                assert!(statuses.contains_key(status), "{path} lacks {status}");
            }
            if operation["x-operation-mutates"] == true {
                assert!(statuses.contains_key("409"), "{path} lacks 409");
            }
        }
    }
}

#[test]
fn metadata_is_the_only_compatibility_advertisement() {
    let (openapi, _) = provenance_codegen::documents();
    assert!(openapi.get("x-protocol-version").is_none());
    assert!(openapi.get("x-provenance-compatibility").is_none());
    let reference = operation(&openapi, "/metadata", "get")["responses"]["200"]["content"]
        ["application/json"]["schema"]["$ref"]
        .as_str()
        .unwrap();
    let envelope = openapi
        .pointer(reference.strip_prefix('#').unwrap())
        .unwrap();
    let data_ref = envelope["properties"]["data"]["$ref"].as_str().unwrap();
    let metadata = openapi
        .pointer(data_ref.strip_prefix('#').unwrap())
        .unwrap();
    let tuple_ref = metadata["properties"]["compatibility"]["$ref"]
        .as_str()
        .unwrap();
    let tuple = openapi
        .pointer(tuple_ref.strip_prefix('#').unwrap())
        .unwrap();
    assert_eq!(
        tuple["required"],
        json!(["wire", "state", "review_journal", "read_derivation"])
    );
}
