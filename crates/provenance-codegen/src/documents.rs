//! Export catalog schemas without sharing names across operation or direction.
use serde_json::{json, Map, Value};

pub fn component(name: &str, mut schema: Value, components: &mut Map<String, Value>) -> Value {
    let definitions = schema.as_object_mut().unwrap().remove("$defs");
    schema.as_object_mut().unwrap().remove("$schema");
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
    let version = provenance_core::protocol::SDK_PROTOCOL_VERSION;
    let mut schemas = Map::new();
    let mut paths = Map::new();
    let mut tools = Vec::new();
    let definitions = provenance_store::operations::catalog::definitions();
    check_operation_names(definitions.iter().map(|definition| definition.name));
    for definition in definitions {
        let input = definition.mcp_input_schema();
        let name = pascal(definition.name);
        let request = component(
            &format!("{name}RequestInput"),
            definition.request_schema.clone(),
            &mut schemas,
        );
        let success = component(
            &format!("{name}SuccessOutput"),
            definition.success_schema.clone(),
            &mut schemas,
        );
        let failure = component(
            &format!("{name}FailureOutput"),
            definition.failure_schema,
            &mut schemas,
        );
        let method = format!("{}{}", &name[..1].to_lowercase(), &name[1..]);
        paths.insert(format!("/v{version}/operations/{}", definition.name), json!({
            "post": {
                "operationId": method,
                "requestBody": {"required":true,"content":{"application/json":{"schema": request}}},
                "responses": {
                    "200":{"description":"Operation result","content":{"application/json":{"schema":success}}},
                    "400":{"description":"Invalid request","content":{"application/json":{"schema":failure}}},
                    "401":{"description":"Authentication required","content":{"application/json":{"schema":failure}}},
                    "403":{"description":"Access denied","content":{"application/json":{"schema":failure}}},
                    "404":{"description":"Unknown operation or target","content":{"application/json":{"schema":failure}}},
                    "500":{"description":"Internal failure","content":{"application/json":{"schema":failure}}},
                    "503":{"description":"Execution resources unavailable","content":{"application/json":{"schema":failure}}}
                }
            }
        }));
        tools.push(json!({"name":definition.name,"inputSchema":input,"outputSchema":definition.success_schema}));
    }
    let settings = schemars::generate::SchemaSettings::draft2020_12()
        .with(|settings| settings.contract = schemars::generate::Contract::Serialize);
    let mut metadata = serde_json::to_value(
        settings
            .into_generator()
            .into_root_schema_for::<provenance_core::protocol::host::HostMetadata>(),
    )
    .unwrap();
    metadata["properties"]["protocol_version"] = json!({"type":"integer","const":version});
    let metadata = component("MetadataOutput", metadata, &mut schemas);
    paths.insert(
        "/metadata".into(),
        json!({"get":{"operationId":"metadata","responses":{"200":{
        "description":"Engine metadata","content":{"application/json":{"schema":metadata}}}}}}),
    );
    (
        json!({"openapi":"3.1.0","info":{"title":"Provenance operations","version":version.to_string()},
        "x-protocol-version":version,"paths":paths,"components":{"schemas":schemas}}),
        json!({"tools":tools}),
    )
}

fn check_operation_names<'a>(names: impl Iterator<Item = &'a str>) {
    let mut converted = std::collections::BTreeSet::new();
    for name in names {
        assert!(
            converted.insert(pascal(name)),
            "converted operation name collision: {name}"
        );
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[should_panic(expected = "converted operation name collision")]
    fn converted_operation_names_cannot_collide() {
        super::check_operation_names(["foo-bar", "fooBar"].into_iter());
    }
}
