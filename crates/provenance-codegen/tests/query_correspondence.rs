use serde_json::Value;

fn operation<'a>(document: &'a Value, path: &str) -> &'a Value {
    &document["paths"][path]["get"]
}

fn variant<'a>(operation: &'a Value, selector: Option<&str>) -> &'a Value {
    operation["x-provenance-query-variants"]
        .as_array()
        .expect("query variants")
        .iter()
        .find(|variant| {
            selector.map_or_else(
                || variant["selector"].is_null(),
                |selector| variant["selector"] == selector,
            )
        })
        .expect("selected query variant")
}

fn parameter<'a>(variant: &'a Value, name: &str) -> &'a Value {
    variant["parameters"]
        .as_array()
        .expect("variant parameters")
        .iter()
        .find(|parameter| parameter["name"] == name)
        .expect("selected parameter")
}

fn references<'a>(value: &'a Value, found: &mut Vec<&'a str>) {
    match value {
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                found.push(reference);
            }
            for child in object.values() {
                references(child, found);
            }
        }
        Value::Array(array) => {
            for child in array {
                references(child, found);
            }
        }
        _ => {}
    }
}

#[test]
fn catalog_query_parameters_retain_required_input_facts() {
    let definitions = provenance_store::operations::catalog::definitions();
    for (definition_name, selector, required) in [
        ("list-rules", "search", "text"),
        ("list-rules", "stale", "base"),
        ("list-rules", "resolve-symbol", "file"),
    ] {
        let definition = definitions
            .iter()
            .find(|definition| definition.name == definition_name)
            .expect("definition");
        let route = definition
            .registration
            .queries
            .iter()
            .find(|route| route.name == selector)
            .expect("query route");
        assert!(
            route
                .parameters
                .iter()
                .any(|parameter| parameter.name == required && parameter.required),
            "{definition_name} query {selector} must require {required}"
        );
    }
}

#[test]
fn openapi_retains_each_query_input_and_result_contract() {
    let (openapi, _) = provenance_codegen::documents();
    let list = operation(&openapi, "/rules");
    let base = variant(list, None);
    let search = variant(list, Some("search"));
    let stale = variant(list, Some("stale"));
    let resolve = variant(list, Some("resolve-symbol"));

    assert_eq!(parameter(search, "query")["required"], true);
    assert_eq!(parameter(search, "query")["schema"]["const"], "search");
    assert_eq!(parameter(search, "text")["required"], true);
    assert_eq!(parameter(stale, "base")["required"], true);
    assert_eq!(parameter(resolve, "file")["required"], true);

    for variant in [base, search, stale, resolve] {
        for contract in ["success", "failure"] {
            let reference = variant[contract]["$ref"]
                .as_str()
                .expect("variant contract reference");
            assert!(openapi.pointer(reference.trim_start_matches('#')).is_some());
        }
    }
    assert_ne!(base["success"], search["success"]);
    assert_ne!(search["success"], stale["success"]);
    assert_ne!(stale["success"], resolve["success"]);
}

#[test]
fn member_queries_keep_path_identity_and_selected_results() {
    let (openapi, _) = provenance_codegen::documents();
    let member = operation(&openapi, "/rules/{id}");
    for selector in [None, Some("neighbors"), Some("trace"), Some("impact")] {
        let selected = variant(member, selector);
        assert_eq!(parameter(selected, "id")["required"], true);
        if let Some(selector) = selector {
            assert_eq!(parameter(selected, "query")["schema"]["const"], selector);
        }
    }
    assert_ne!(
        variant(member, Some("neighbors"))["success"],
        variant(member, Some("trace"))["success"]
    );
}

#[test]
fn query_variants_reuse_matching_consolidated_response_components() {
    let (openapi, _) = provenance_codegen::documents();
    let selected = variant(operation(&openapi, "/rules/{id}"), Some("trace"));
    let schemas = openapi["components"]["schemas"]
        .as_object()
        .expect("component schemas");

    for (contract, consolidated) in [("success", "GetRuleSuccess"), ("failure", "GetRuleFailure")]
    {
        let name = selected[contract]["$ref"]
            .as_str()
            .expect("variant contract reference")
            .trim_start_matches("#/components/schemas/");
        let mut found = Vec::new();
        references(&schemas[name], &mut found);
        assert!(!found.is_empty(), "variant contract must remain explicit");
        assert!(
            found.iter().any(|reference| reference.starts_with(&format!(
                "#/components/schemas/{consolidated}"
            ))),
            "{name} must reuse matching {consolidated} components: {found:?}"
        );
    }
    assert!(!schemas.contains_key("GetRuleTraceSuccessResponseMetaFreshnessCause"));
    assert!(!schemas.contains_key("GetRuleTraceFailureOperationError"));
}
