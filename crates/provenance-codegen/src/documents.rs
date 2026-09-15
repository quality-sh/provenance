//! Export the registered resource surface as `OpenAPI` and MCP documents.
use serde_json::{json, Map, Value};

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
    let mut paths: Map<String, Value> = Map::new();
    let mut tools = Vec::new();
    let definitions = provenance_store::operations::catalog::definitions();
    check_names(&definitions);
    for definition in definitions {
        let family = pascal(definition.name);
        let request = definition
            .request_schema
            .clone()
            .map(|schema| component(&format!("{family}Request"), schema, &mut schemas));
        let success = component(
            &format!("{family}Success"),
            definition.success_schema.clone(),
            &mut schemas,
        );
        let failure = component(
            &format!("{family}Failure"),
            definition.failure_schema.clone(),
            &mut schemas,
        );
        let parameters = definition
            .parameters
            .iter()
            .map(|parameter| {
                json!({
                    "name": parameter.name,
                    "in": parameter.location,
                    "required": parameter.required,
                    "schema": parameter.schema,
                })
            })
            .collect::<Vec<_>>();
        let mut responses = Map::new();
        let mut success_response = json!({"description":"Operation result",
            "content":{"application/json":{"schema":success}}});
        if definition.returns_etag() {
            success_response["headers"] = json!({"ETag":{"required":true,"schema":{"type":"string"},
                "description":"The current resource precondition token."}});
        }
        responses.insert("200".into(), success_response);
        for status in &definition.http_statuses {
            responses.insert(
                status.to_string(),
                json!({"description":"Operation failed or was refused",
                "content":{"application/json":{"schema":failure}}}),
            );
        }
        let mut operation = json!({
            "operationId": definition.operation_id,
            "description": definition.description,
            "x-operation-mutates": definition.mutates,
            "parameters": parameters,
            "responses": responses,
        });
        if let Some(request) = request {
            operation["requestBody"] = json!({"required":true,
                "content":{"application/json":{"schema":request}}});
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
            "x-operation-mutates": definition.mutates,
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
    for status in [400, 401, 403, 404, 409, 500, 503] {
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
    #[test]
    #[should_panic(expected = "operationId collision")]
    fn operation_ids_cannot_collide() {
        let mut definitions = provenance_store::operations::catalog::definitions();
        definitions[1].operation_id = definitions[0].operation_id;
        super::check_names(&definitions[..2]);
    }
}
