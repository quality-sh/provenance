//! The exported contract repeats the bound request and response types instead
//! of a broad placeholder. Path parameters carry their typed field's
//! constraints, paging responses carry the producer's own page facts, and
//! metadata-free responses stay valid.

use serde_json::{json, Value};

fn operation<'a>(document: &'a Value, path: &str, method: &str) -> &'a Value {
    &document["paths"][path][method]
}

fn parameter<'a>(operation: &'a Value, name: &str) -> &'a Value {
    operation["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|parameter| parameter["name"] == name && parameter["in"] == "path")
        .unwrap_or_else(|| panic!("missing path parameter {name}"))
}

fn success<'a>(document: &'a Value, path: &str, method: &str) -> &'a Value {
    let reference = operation(document, path, method)["responses"]["200"]["content"]
        ["application/json"]["schema"]["$ref"]
        .as_str()
        .unwrap();
    document
        .pointer(reference.strip_prefix('#').unwrap())
        .unwrap()
}

#[test]
fn path_parameters_carry_the_stable_id_pattern_across_families() {
    let (openapi, _) = provenance_codegen::documents();
    let stable_id = json!({
        "description": "A stable artifact id. The inner `String` is private and `new` is the only way in, so every `StableId` in existence satisfies [`is_well_formed_id`].",
        "pattern": "^[a-z0-9_-]+$",
        "type": "string",
    });
    for (path, method, name) in [
        ("/sources/{id}", "get", "id"),
        ("/sources/{id}", "patch", "id"),
        ("/requirements/{id}/history/{entry_id}", "get", "id"),
        ("/requirements/{id}/history/{entry_id}", "get", "entry_id"),
        ("/proposals/{id}/dispositions/{fact_id}", "get", "fact_id"),
        (
            "/sources/{id}/discussions/{discussion_id}/messages/{message_id}",
            "get",
            "message_id",
        ),
        ("/topics/{id}/discussions", "get", "id"),
        (
            "/questions/{id}/discussion-containers/{container_id}/legacy-messages",
            "get",
            "container_id",
        ),
        (
            "/requirements/{id}/submissions/{proposal_id}/decide",
            "post",
            "proposal_id",
        ),
    ] {
        let parameter = parameter(operation(&openapi, path, method), name);
        assert_eq!(
            parameter["schema"]["pattern"], stable_id["pattern"],
            "{method} {path} {name}"
        );
        assert_eq!(
            parameter["schema"]["type"], "string",
            "{method} {path} {name}"
        );
        assert!(
            parameter["schema"].get("description").is_some(),
            "{method} {path} {name} keeps the type's own statement"
        );
    }
}

#[test]
fn the_evidence_side_parameter_publishes_the_closed_enum() {
    let (openapi, mcp) = provenance_codegen::documents();
    let path = "/requirements/{id}/history/{entry_id}/evidence/{side}";
    let parameter = parameter(operation(&openapi, path, "get"), "side");
    assert_eq!(
        parameter["schema"]["enum"],
        json!(["before", "after"]),
        "the published side selection is closed"
    );
    for tool in mcp["tools"].as_array().unwrap() {
        if tool["name"] != "get-requirement-history-evidence" {
            continue;
        }
        let input = &tool["inputSchema"]["properties"];
        assert_eq!(input["side"]["enum"], json!(["before", "after"]));
        assert_eq!(input["side"]["type"], "string");
    }
}

#[test]
fn every_path_parameter_publishes_a_derived_schema() {
    let (openapi, _) = provenance_codegen::documents();
    for (path, item) in openapi["paths"].as_object().unwrap() {
        for (method, operation) in item.as_object().unwrap() {
            for parameter in operation["parameters"].as_array().unwrap() {
                if parameter["in"] != "path" {
                    continue;
                }
                assert_ne!(
                    parameter["schema"],
                    Value::Null,
                    "{method} {path} publishes the manual placeholder for {}",
                    parameter["name"]
                );
                assert!(
                    parameter["schema"].get("type").is_some(),
                    "{method} {path} {}: no derived type",
                    parameter["name"]
                );
            }
        }
    }
}

#[test]
fn paging_successes_require_the_producer_page_facts_in_meta() {
    let (openapi, _) = provenance_codegen::documents();
    let list = success(&openapi, "/sources", "get");
    let variants = list["anyOf"]
        .as_array()
        .map_or_else(|| vec![list], |v| v.iter().collect());
    for variant in variants {
        let meta = &variant["properties"]["meta"];
        assert_eq!(meta["required"], json!(["limit", "has_more"]));
        assert_eq!(meta["properties"]["limit"]["type"], "integer");
        assert_eq!(meta["properties"]["has_more"]["type"], "boolean");
        assert_eq!(
            meta["properties"]["next_cursor"]["type"],
            json!(["string", "null"])
        );
    }
}

#[test]
fn member_successes_keep_a_metadata_free_meta() {
    let (openapi, _) = provenance_codegen::documents();
    let member = success(&openapi, "/sources/{id}", "get");
    let base = member["anyOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|variant| {
            let reference = variant["properties"]["data"]["$ref"].as_str().unwrap();
            let data = openapi
                .pointer(reference.strip_prefix('#').unwrap())
                .unwrap();
            data["properties"].get("url").is_some()
        })
        .expect("the member variant");
    let meta = &base["properties"]["meta"];
    assert!(
        meta.get("required").is_none(),
        "a member meta stays optional: {meta}"
    );
    assert_eq!(
        meta["properties"]["limit"]["type"],
        json!(["integer", "null"])
    );
}

#[test]
fn query_answers_carry_their_own_page_facts() {
    let (openapi, _) = provenance_codegen::documents();
    let trace = success(&openapi, "/sources/{id}", "get");
    let trace = trace["anyOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|variant| {
            variant["properties"]["data"]["properties"]
                .get("nodes")
                .is_some()
        })
        .expect("trace variant");
    let meta = &trace["properties"]["meta"];
    assert_eq!(meta["required"], json!(["limit", "has_more"]));
    assert!(
        meta["properties"].get("next_cursor").is_none(),
        "trace answers never ship a cursor: {meta}"
    );
}

#[test]
fn failure_envelopes_keep_the_optional_meta() {
    let (openapi, _) = provenance_codegen::documents();
    let reference = operation(&openapi, "/sources", "get")["responses"]["400"]["content"]
        ["application/json"]["schema"]["$ref"]
        .as_str()
        .unwrap();
    let failure = openapi
        .pointer(reference.strip_prefix('#').unwrap())
        .unwrap();
    let branches = failure["oneOf"]
        .as_array()
        .or_else(|| failure["anyOf"].as_array())
        .map_or_else(|| vec![failure], |branches| branches.iter().collect());
    for variant in branches {
        let meta = &variant["properties"]["meta"];
        let reference = meta.get("$ref").and_then(Value::as_str);
        let meta = reference.map_or(meta, |reference| {
            openapi
                .pointer(reference.strip_prefix('#').unwrap())
                .unwrap()
        });
        assert!(
            meta.get("required").is_none(),
            "failure meta stays optional: {meta}"
        );
    }
}
