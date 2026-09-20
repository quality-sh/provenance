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
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    properties
        .iter()
        .filter(|(name, _)| !is_bound(name, route, query))
        .map(|(name, field)| {
            let parameter_schema = parameter_schema(schema, field)
                .unwrap_or_else(|| panic!("unsupported unbound query parameter `{name}`: {field}"));
            Parameter {
                name: Box::leak(name.clone().into_boxed_str()),
                location: "query",
                required: required.contains(name.as_str()),
                schema: parameter_schema,
            }
        })
        .collect()
}

fn is_bound(name: &str, route: &RequestBinding, query: &QueryRequestBinding) -> bool {
    name == "protocol_version"
        || route.path.iter().any(|binding| binding.field == name)
        || route.scope_field == Some(name)
        || query.node_type.is_some() && matches!(name, "node_type" | "node_types")
}

fn parameter_schema(root: &Value, field: &Value) -> Option<Value> {
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
        if field.get("default").is_some_and(Value::is_null) {
            field.as_object_mut().unwrap().remove("default");
        }
        return supported_schema(root, &field);
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
            .then(|| parameter_schema(root, variants[0]))
            .flatten();
    }
    supported_schema(root, field)
}

fn supported_schema(root: &Value, field: &Value) -> Option<Value> {
    if is_scalar(field) {
        return Some(field.clone());
    }
    if field.get("type").and_then(Value::as_str) != Some("array") {
        return None;
    }
    let mut array = field.clone();
    let items = parameter_schema(root, field.get("items")?)?;
    if !is_scalar(&items) {
        return None;
    }
    array["items"] = items;
    Some(array)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::catalog::{QueryRequestBinding, RequestBinding};
    use serde_json::json;

    #[test]
    #[should_panic(expected = "unsupported unbound query parameter")]
    fn unsupported_query_fields_refuse_catalog_construction() {
        let schema = json!({
            "type": "object",
            "properties": {"unsupported": {"type": "object"}}
        });
        parameters(
            &schema,
            &RequestBinding::default(),
            &QueryRequestBinding::default(),
        );
    }
}
