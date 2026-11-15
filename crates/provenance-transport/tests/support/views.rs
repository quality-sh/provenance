//! An answer with each generated value named, and its stable view.

use super::names::Names;
use serde_json::{json, Map, Value};

/// The answer with each generated value named, and each `created_at`
/// number as `<time>`.
pub fn named(names: &mut Names, value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(names.name(text)),
        Value::Array(items) => Value::Array(items.iter().map(|item| named(names, item)).collect()),
        Value::Object(fields) => {
            let mut out = Map::new();
            for (key, field) in fields {
                let value = if key == "created_at" && field.is_number() {
                    json!("<time>")
                } else {
                    named(names, field)
                };
                out.insert(key.clone(), value);
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// The transcript without what differs from run to run. Record ids such as
/// proposal ids are random, so each generated value becomes one word. A list
/// of records keyed by a generated id is in id order, so it is sorted by its
/// text.
pub fn stable_view(value: &Value) -> Value {
    match value {
        Value::String(text) if text.starts_with("<generated ") => json!("<generated>"),
        Value::Array(items) => {
            let mut items: Vec<Value> = items.iter().map(stable_view).collect();
            let keyed = |item: &Value| item.get("id") == Some(&json!("<generated>"));
            if !items.is_empty() && items.iter().all(keyed) {
                items.sort_by_key(|item| serde_json::to_string(item).unwrap());
            }
            Value::Array(items)
        }
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, field)| (key.clone(), stable_view(field)))
                .collect(),
        ),
        other => other.clone(),
    }
}
