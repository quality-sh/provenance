//! Export the registered resource surface as `OpenAPI` and MCP documents.
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub fn component(name: &str, mut schema: Value, components: &mut Map<String, Value>) -> Value {
    let definitions = schema.as_object_mut().unwrap().remove("$defs");
    schema.as_object_mut().unwrap().remove("$schema");
    schema["x-provenance-model-family"] = json!(name);
    strip_titles(&mut schema);
    rewrite(&mut schema, name);
    if let Some(Value::Object(definitions)) = definitions {
        for (key, mut value) in definitions {
            rewrite(&mut value, name);
            assert!(
                components.insert(format!("{name}{key}"), value).is_none(),
                "schema component name collision: {name}{key}"
            );
        }
    }
    assert!(
        components.insert(name.to_owned(), schema).is_none(),
        "schema component name collision: {name}"
    );
    json!({"$ref": format!("#/components/schemas/{name}")})
}

fn strip_titles(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if object.get("title").is_some_and(Value::is_string) {
                object.remove("title");
            }
            for child in object.values_mut() {
                strip_titles(child);
            }
        }
        Value::Array(array) => {
            for child in array {
                strip_titles(child);
            }
        }
        _ => {}
    }
}

fn rewrite(value: &mut Value, prefix: &str) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref") {
                if let Some(name) = reference.strip_prefix("#/$defs/") {
                    *reference = format!("#/components/schemas/{prefix}{name}");
                }
            }
            for value in object.values_mut() {
                rewrite(value, prefix);
            }
        }
        Value::Array(array) => {
            for value in array {
                rewrite(value, prefix);
            }
        }
        _ => {}
    }
}

pub fn pascal(name: &str) -> String {
    name.split('-')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .unwrap()
                .to_uppercase()
                .chain(chars)
                .collect::<String>()
        })
        .collect()
}

pub fn documents() -> (Value, Value) {
    let mut schemas = Map::new();
    let mut shared_components = BTreeMap::new();
    let mut paths: Map<String, Value> = Map::new();
    let mut tools = Vec::new();
    let definitions = provenance_store::operations::catalog::definitions();
    check_names(definitions);
    for definition in definitions {
        let family = pascal(definition.name);
        let variants = definition.query_variants();
        let share = variants.is_empty()
            && shared_review_handler(definition.registration.handler.operation)
            && !definition.name.contains("requirement");
        let request = definition.request_schema().cloned().map(|schema| {
            route_component(
                definition,
                "request",
                &format!("{family}Request"),
                schema,
                share,
                &mut shared_components,
                &mut schemas,
            )
        });
        let success = route_component(
            definition,
            "success",
            &format!("{family}Success"),
            definition.success_schema(),
            share,
            &mut shared_components,
            &mut schemas,
        );
        let failure = route_component(
            definition,
            "failure",
            &format!("{family}Failure"),
            definition.failure_schema().clone(),
            share,
            &mut shared_components,
            &mut schemas,
        );
        let parameters = definition
            .parameters()
            .iter()
            .map(parameter_document)
            .collect::<Vec<_>>();
        let mut responses = Map::new();
        let mut success_response = json!({"description":"Operation result",
            "content":{"application/json":{"schema":success}}});
        if definition.returns_etag() {
            success_response["headers"] = json!({"ETag":{"required":true,"schema":{"type":"string"},
                "description":"The current resource precondition token."}});
        }
        responses.insert("200".into(), success_response);
        for status in definition.http_statuses() {
            responses.insert(
                status.to_string(),
                json!({"description":"Operation failed or was refused",
                "content":{"application/json":{"schema":failure}}}),
            );
        }
        let mut operation = json!({
            "operationId": definition.operation_id,
            "description": definition.description,
            "x-operation-mutates": definition.mutates(),
            "parameters": parameters,
            "responses": responses,
        });
        if let Some(request) = request {
            operation["requestBody"] = json!({"required":true,
                "content":{"application/json":{"schema":request}}});
        }
        if !variants.is_empty() {
            operation["x-provenance-query-variants"] =
                Value::Array(query_variant_documents(variants, &family, &mut schemas));
        }
        paths
            .entry(definition.path.to_owned())
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .unwrap()
            .insert(definition.method.as_str().into(), operation);
        tools.push(json!({
            "name": definition.name,
            "description": definition.description,
            "inputSchema": definition.mcp_input_schema(),
            "outputSchema": definition.mcp_output_schema(),
            "x-operation-mutates": definition.mutates(),
        }));
    }
    add_metadata(&mut paths, &mut schemas);
    (
        json!({"openapi":"3.1.0","info":{"title":"Provenance resource contract",
            "version":env!("CARGO_PKG_VERSION")},
            "paths":paths,"components":{"schemas":schemas}}),
        json!({"tools":tools}),
    )
}

fn shared_review_handler(handler: &str) -> bool {
    matches!(
        handler,
        "submit-record-review"
            | "decide-record-review"
            | "withdraw-record-review"
            | "review-history"
            | "review-history-entry"
            | "review-evidence"
            | "get-reviewed-resource"
    )
}

fn route_component(
    definition: &provenance_store::operations::catalog::Definition,
    role: &str,
    name: &str,
    schema: Value,
    share: bool,
    shared: &mut BTreeMap<(String, String, String), Value>,
    components: &mut Map<String, Value>,
) -> Value {
    if !share {
        return component(name, schema, components);
    }
    let key = (
        definition.registration.handler.operation.to_owned(),
        role.to_owned(),
        schema.to_string(),
    );
    if let Some(reference) = shared.get(&key) {
        return reference.clone();
    }
    let reference = component(name, schema, components);
    shared.insert(key, reference.clone());
    reference
}

fn query_variant_documents(
    variants: Vec<provenance_store::operations::catalog::QueryVariant>,
    family: &str,
    schemas: &mut Map<String, Value>,
) -> Vec<Value> {
    variants
        .into_iter()
        .map(|variant| {
            let suffix = variant.selector.map_or_else(|| "Base".to_owned(), pascal);
            let success = query_variant_component(
                &format!("{family}{suffix}Success"),
                &format!("{family}Success"),
                variant.success_schema,
                schemas,
            );
            let failure = query_variant_component(
                &format!("{family}{suffix}Failure"),
                &format!("{family}Failure"),
                variant.failure_schema,
                schemas,
            );
            json!({
                "selector": variant.selector,
                "parameters": variant.parameters.iter().map(parameter_document).collect::<Vec<_>>(),
                "success": success,
                "failure": failure,
            })
        })
        .collect()
}

fn query_variant_component(
    name: &str,
    consolidated: &str,
    mut schema: Value,
    components: &mut Map<String, Value>,
) -> Value {
    schema.as_object_mut().unwrap().remove("$schema");
    schema["x-provenance-model-family"] = json!(name);
    strip_titles(&mut schema);
    let definitions = schema
        .as_object_mut()
        .unwrap()
        .remove("$defs")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let mut shared = definitions
        .iter()
        .filter_map(|(key, value)| {
            let mut candidate = value.clone();
            rewrite(&mut candidate, consolidated);
            (components.get(&format!("{consolidated}{key}")) == Some(&candidate))
                .then(|| key.clone())
        })
        .collect::<BTreeSet<_>>();
    loop {
        let dependent = shared
            .iter()
            .find(|key| !references_only_shared(&definitions[*key], &shared))
            .cloned();
        let Some(dependent) = dependent else {
            break;
        };
        shared.remove(&dependent);
    }
    let targets = definitions
        .keys()
        .map(|key| {
            let prefix = if shared.contains(key) {
                consolidated
            } else {
                name
            };
            (key.clone(), format!("{prefix}{key}"))
        })
        .collect::<BTreeMap<_, _>>();
    rewrite_variant(&mut schema, &targets);
    for (key, mut value) in definitions {
        if shared.contains(&key) {
            continue;
        }
        rewrite_variant(&mut value, &targets);
        let target = targets[&key].clone();
        assert!(
            components.insert(target.clone(), value).is_none(),
            "schema component name collision: {target}"
        );
    }
    assert!(
        components.insert(name.to_owned(), schema).is_none(),
        "schema component name collision: {name}"
    );
    json!({"$ref": format!("#/components/schemas/{name}")})
}

fn references_only_shared(value: &Value, shared: &BTreeSet<String>) -> bool {
    match value {
        Value::Object(object) => {
            let reference_is_shared = object
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|reference| reference.strip_prefix("#/$defs/"))
                .is_none_or(|name| shared.contains(name));
            reference_is_shared
                && object
                    .values()
                    .all(|child| references_only_shared(child, shared))
        }
        Value::Array(array) => array
            .iter()
            .all(|child| references_only_shared(child, shared)),
        _ => true,
    }
}

fn rewrite_variant(value: &mut Value, targets: &BTreeMap<String, String>) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref") {
                if let Some(name) = reference.strip_prefix("#/$defs/") {
                    *reference = format!("#/components/schemas/{}", targets[name]);
                }
            }
            for child in object.values_mut() {
                rewrite_variant(child, targets);
            }
        }
        Value::Array(array) => {
            for child in array {
                rewrite_variant(child, targets);
            }
        }
        _ => {}
    }
}

fn parameter_document(parameter: &provenance_store::operations::catalog::Parameter) -> Value {
    let mut document = json!({
        "name": parameter.name,
        "in": parameter.location,
        "required": parameter.required,
        "schema": parameter.schema,
    });
    if parameter.location == "query" && parameter.schema["type"] == "array" {
        document["style"] = json!("form");
        document["explode"] = json!(false);
    }
    document
}

fn add_metadata(paths: &mut Map<String, Value>, schemas: &mut Map<String, Value>) {
    use provenance_core::protocol::{
        failure::{FailureEnvelope, OperationFailure},
        host::HostMetadata,
        SuccessEnvelope,
    };
    use schemars::generate::{Contract, SchemaSettings};
    let success_schema = serde_json::to_value(
        SchemaSettings::draft2020_12()
            .with(|s| s.contract = Contract::Serialize)
            .into_generator()
            .into_root_schema_for::<SuccessEnvelope<HostMetadata>>(),
    )
    .unwrap();
    let success = component("MetadataSuccess", success_schema, schemas);
    let failure_schema = serde_json::to_value(
        SchemaSettings::draft2020_12()
            .with(|s| s.contract = Contract::Serialize)
            .into_generator()
            .into_root_schema_for::<FailureEnvelope<OperationFailure>>(),
    )
    .unwrap();
    let failure = component("MetadataFailure", failure_schema, schemas);
    let mut responses = Map::new();
    responses.insert(
        "200".into(),
        json!({"description":"Operation result","content":{"application/json":{"schema":success}}}),
    );
    for status in [400, 401, 403, 404, 405, 409, 500, 503] {
        responses.insert(status.to_string(), json!({"description":"Operation failed or was refused","content":{"application/json":{"schema":failure}}}));
    }
    paths.insert(
        "/metadata".into(),
        json!({"get":{"operationId":"metadata",
        "description":"Read the compatibility tuple and bound connection facts.",
        "x-operation-mutates":false,"parameters":[],"responses":responses}}),
    );
}

fn check_names(definitions: &[provenance_store::operations::catalog::Definition]) {
    let mut operations = std::collections::BTreeSet::new();
    let mut tools = std::collections::BTreeSet::new();
    for definition in definitions {
        assert!(
            operations.insert(definition.operation_id),
            "operationId collision: {}",
            definition.operation_id
        );
        assert!(
            tools.insert(definition.name),
            "MCP tool name collision: {}",
            definition.name
        );
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    fn schema_reference<'a>(document: &'a Value, pointer: &str) -> &'a Value {
        document
            .pointer(pointer)
            .unwrap_or_else(|| panic!("missing schema reference: {pointer}"))
    }

    #[test]
    #[should_panic(expected = "operationId collision")]
    fn operation_ids_cannot_collide() {
        let mut definitions = provenance_store::operations::catalog::definitions().to_vec();
        definitions[1].operation_id = definitions[0].operation_id;
        super::check_names(&definitions[..2]);
    }

    #[test]
    fn routes_for_one_handler_share_identical_models() {
        let (openapi, _) = super::documents();
        let source = "/paths/~1sources~1{id}~1submit/post";
        let domain = "/paths/~1domains~1{id}~1submit/post";

        for suffix in [
            "/requestBody/content/application~1json/schema",
            "/responses/200/content/application~1json/schema",
            "/responses/409/content/application~1json/schema",
        ] {
            assert_eq!(
                schema_reference(&openapi, &format!("{source}{suffix}")),
                schema_reference(&openapi, &format!("{domain}{suffix}")),
            );
        }
    }
}
