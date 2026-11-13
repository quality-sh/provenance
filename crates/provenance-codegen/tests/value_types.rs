//! An implementation aid with no Rule binding. Each schema of the generated contract accepts a
//! value, and each closed object in it has a field. The Rust models of the HTTP client come
//! from these schemas, so each model has a variant or a field.

use serde_json::Value;

/// Adds each schema in `schema` that accepts no value, and each closed object that has no
/// field.
fn faults(schema: &Value, at: &str, found: &mut Vec<String>) {
    if schema == &Value::Bool(false) {
        found.push(format!("{at}: no value satisfies this schema"));
        return;
    }
    let Some(object) = schema.as_object() else {
        return;
    };
    let children = |key: &str| object.get(key).and_then(Value::as_object);
    let closed = object.get("type") == Some(&Value::from("object"))
        && object.get("additionalProperties") == Some(&Value::Bool(false));
    if closed && children("properties").is_none_or(serde_json::Map::is_empty) {
        found.push(format!("{at}: closed object with no field"));
    }
    for key in ["properties", "$defs"] {
        for (name, child) in children(key).into_iter().flatten() {
            faults(child, &format!("{at}/{key}/{name}"), found);
        }
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        let variants = object.get(key).and_then(Value::as_array);
        for (index, child) in variants.into_iter().flatten().enumerate() {
            faults(child, &format!("{at}/{key}/{index}"), found);
        }
    }
    if let Some(items) = object.get("items").filter(|items| items.is_object()) {
        faults(items, &format!("{at}/items"), found);
    }
}

/// Adds each enum with no variant and each struct with no field.
fn models_without_value(items: &[syn::Item], path: &str, found: &mut Vec<String>) {
    for item in items {
        match item {
            syn::Item::Enum(model) if model.variants.is_empty() => {
                found.push(format!("{path}: enum {}", model.ident));
            }
            syn::Item::Struct(model) if model.fields.is_empty() => {
                found.push(format!("{path}: struct {}", model.ident));
            }
            syn::Item::Mod(module) => {
                if let Some((_, items)) = &module.content {
                    models_without_value(items, path, found);
                }
            }
            _ => {}
        }
    }
}

#[test]
fn each_contract_schema_accepts_a_value_and_each_closed_object_has_a_field() {
    let (openapi, mcp) = provenance_codegen::documents();
    let mut found = Vec::new();
    for (name, schema) in openapi["components"]["schemas"].as_object().unwrap() {
        faults(schema, name, &mut found);
    }
    for (path, route) in openapi["paths"].as_object().unwrap() {
        for (method, operation) in route.as_object().unwrap() {
            let parameters = operation["parameters"].as_array();
            for parameter in parameters.into_iter().flatten() {
                let at = format!("{method} {path} parameter {}", parameter["name"]);
                faults(&parameter["schema"], &at, &mut found);
            }
        }
    }
    for tool in mcp["tools"].as_array().unwrap() {
        for key in ["inputSchema", "outputSchema"] {
            faults(&tool[key], &format!("{} {key}", tool["name"]), &mut found);
        }
    }
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn each_rust_model_of_the_client_has_a_variant_or_a_field() {
    let (openapi, _) = provenance_codegen::documents();
    let files = provenance_codegen::rust_types(&openapi).unwrap();
    assert!(files.keys().any(|path| path.starts_with("models/")));
    let mut found = Vec::new();
    for (path, source) in &files {
        let file = syn::parse_file(source).unwrap();
        models_without_value(&file.items, path, &mut found);
    }
    assert!(found.is_empty(), "{found:#?}");
}
