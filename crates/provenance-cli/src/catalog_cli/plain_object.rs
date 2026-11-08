//! A plain flag value for an object field that has one required text field.
//!
//! The archive commit is the object `{"commit": "<hash>"}` with an optional
//! time. A plain `--archived-in-commit <hash>` fills the one required field.
//! The JSON flag stays the only form that can also set the other fields.

use provenance_macros::rule;
use serde_json::{Map, Value};

/// Wrap a plain value in its object when the object schema has exactly one
/// required field and that field admits text. Other object schemas give `None`.
#[rule("rule_cli_archive_commit_plain_flag")]
pub(super) fn wrap(root: &Value, schema: &Value, raw: &str) -> Option<Value> {
    let required = sole_required_text_field(root, schema, 0)?;
    let mut object = Map::new();
    object.insert(required, Value::String(raw.to_owned()));
    Some(Value::Object(object))
}

fn sole_required_text_field(root: &Value, schema: &Value, depth: usize) -> Option<String> {
    if depth > 64 {
        return None;
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let target = root
            .get("$defs")?
            .get(reference.strip_prefix("#/$defs/")?)?;
        return sole_required_text_field(root, target, depth + 1);
    }
    if let Some(variants) = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)
    {
        let mut objects = variants
            .iter()
            .filter(|variant| variant.get("type") != Some(&Value::from("null")));
        let only = objects.next()?;
        return match objects.next() {
            Some(_) => None,
            None => sole_required_text_field(root, only, depth + 1),
        };
    }
    if schema.get("type").and_then(Value::as_str) != Some("object") {
        return None;
    }
    let [name] = schema.get("required")?.as_array()?.as_slice() else {
        return None;
    };
    let name = name.as_str()?;
    let field = schema.get("properties")?.get(name)?;
    admits_text(field).then(|| name.to_owned())
}

fn admits_text(schema: &Value) -> bool {
    match schema.get("type") {
        Some(Value::String(kind)) => kind == "string",
        Some(Value::Array(kinds)) => kinds.iter().any(|kind| kind == "string"),
        _ => false,
    }
}
