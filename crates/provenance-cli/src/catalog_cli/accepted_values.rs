//! The refusal of a flag value that the field schema does not accept.

use serde_json::Value;

/// Refuses one flag value and lists the accepted values when the field
/// schema names a closed set of words.
pub(super) fn invalid_value(root: &Value, schema: &Value, flag: &str) -> anyhow::Error {
    let mut values = Vec::new();
    collect(root, schema, 0, &mut values);
    if values.is_empty() {
        return anyhow::anyhow!("invalid value for --{flag}");
    }
    anyhow::anyhow!(
        "invalid value for --{flag}; accepted values: {}",
        values.join(", ")
    )
}

fn collect(root: &Value, schema: &Value, depth: usize, values: &mut Vec<String>) {
    if depth > 64 {
        return;
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        if let Some(target) = super::input::resolve(root, reference) {
            collect(root, target, depth + 1, values);
        }
        return;
    }
    let words = schema
        .get("enum")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .chain(schema.get("const"));
    for word in words.filter_map(Value::as_str) {
        if !values.iter().any(|value| value == word) {
            values.push(word.to_owned());
        }
    }
    for key in ["anyOf", "oneOf"] {
        for variant in schema
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            collect(root, variant, depth + 1, values);
        }
    }
}
