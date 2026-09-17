use crate::operations::catalog::{Parameter, QueryRequestBinding, RequestBinding};
use serde_json::Value;

pub(super) fn parameters(
    schema: &Value,
    route: &RequestBinding,
    query: &QueryRequestBinding,
) -> Vec<Parameter> {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Vec::new();
    };
    properties
        .iter()
        .filter(|(name, _)| !is_bound(name, route, query))
        .filter_map(|(name, field)| {
            scalar_schema(schema, field).map(|schema| Parameter {
                name: Box::leak(name.clone().into_boxed_str()),
                location: "query",
                required: false,
                schema,
            })
        })
        .collect()
}

fn is_bound(name: &str, route: &RequestBinding, query: &QueryRequestBinding) -> bool {
    name == "protocol_version"
        || route.path.iter().any(|binding| binding.field == name)
        || route.scope_field == Some(name)
        || query.node_type.is_some() && matches!(name, "node_type" | "node_types")
}

fn scalar_schema(root: &Value, field: &Value) -> Option<Value> {
    let field = resolve(root, field)?;
    if let Some(types) = field.get("type").and_then(Value::as_array) {
        let types = types
            .iter()
            .filter(|kind| kind.as_str() != Some("null"))
            .cloned()
            .collect::<Vec<_>>();
        if types.len() != 1 {
            return None;
        }
        let mut field = field.clone();
        field["type"] = types[0].clone();
        return is_scalar(&field).then_some(field);
    }
    if let Some(variants) = field
        .get("anyOf")
        .or_else(|| field.get("oneOf"))
        .and_then(Value::as_array)
    {
        let variants = variants
            .iter()
            .filter(|variant| variant.get("type").and_then(Value::as_str) != Some("null"))
            .collect::<Vec<_>>();
        return (variants.len() == 1)
            .then(|| scalar_schema(root, variants[0]))
            .flatten();
    }
    is_scalar(field).then(|| field.clone())
}

fn resolve<'a>(root: &'a Value, field: &'a Value) -> Option<&'a Value> {
    let Some(reference) = field.get("$ref").and_then(Value::as_str) else {
        return Some(field);
    };
    let name = reference.strip_prefix("#/$defs/")?;
    root.get("$defs")?.get(name)
}

fn is_scalar(schema: &Value) -> bool {
    matches!(
        schema.get("type").and_then(Value::as_str),
        Some("string" | "boolean" | "integer" | "number")
    )
}
