//! Adapt `OpenAPI` 3.1 component schemas for the pinned Rust generator.
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn draft7(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("$schema");
            if let Some(constant) = map.remove("const") {
                map.insert("enum".into(), json!([constant]));
            }
            if let Some(Value::String(reference)) = map.get_mut("$ref") {
                if let Some(name) = reference.strip_prefix("#/components/schemas/") {
                    *reference = format!("#/definitions/{name}");
                }
            }
            for value in map.values_mut() {
                draft7(value);
            }
        }
        Value::Array(array) => {
            for value in array {
                draft7(value);
            }
        }
        _ => {}
    }
}

pub fn rust_types(
    document: &Value,
) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let mut definitions = document["components"]["schemas"].clone();
    let mut identifiers = std::collections::BTreeSet::new();
    for name in definitions
        .as_object()
        .ok_or("missing OpenAPI components")?
        .keys()
    {
        let normalized: String = name
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        if normalized.is_empty() || !identifiers.insert(normalized) {
            return Err(format!("converted Rust schema name collision: {name}").into());
        }
    }
    crate::unions::normalize(&mut definitions, document);
    draft7(&mut definitions);
    let root = serde_json::from_value::<schemars08::schema::RootSchema>(
        json!({"definitions":definitions}),
    )?;
    let mut types = typify::TypeSpace::new(&typify::TypeSpaceSettings::default());
    types.add_root_schema(root)?;
    let parsed = syn::parse2::<syn::File>(types.to_stream())?;
    crate::model_layout::render(parsed, document)
}
