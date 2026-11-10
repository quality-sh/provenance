//! What one field schema of a request accepts, from one walk of the schema.
//!
//! The walk follows each `$ref` into the request definitions. It goes into
//! each branch of `anyOf`, or of `oneOf` when the schema has no `anyOf`. A
//! reference that does not resolve, or a walk deeper than 64 levels, gives a
//! branch that accepts nothing.

use serde_json::Value;

const MAX_DEPTH: usize = 64;

/// The schema that accepts no value.
static NOTHING: Value = Value::Bool(false);

/// The branches of one field schema: each schema with no reference and no union.
fn branches<'a>(root: &'a Value, schema: &'a Value) -> Vec<&'a Value> {
    let mut found = Vec::new();
    walk(root, schema, 0, &mut found);
    found
}

fn walk<'a>(root: &'a Value, schema: &'a Value, depth: usize, found: &mut Vec<&'a Value>) {
    if depth > MAX_DEPTH {
        found.push(&NOTHING);
        return;
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        match resolve(root, reference) {
            Some(target) => walk(root, target, depth + 1, found),
            None => found.push(&NOTHING),
        }
        return;
    }
    if let Some(variants) = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)
    {
        for variant in variants {
            walk(root, variant, depth + 1, found);
        }
        return;
    }
    found.push(schema);
}

fn resolve<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    let name = reference.strip_prefix("#/$defs/")?;
    root.get("$defs")?.get(name)
}

fn has_type(branch: &Value, wanted: &str) -> bool {
    match branch.get("type") {
        Some(Value::String(kind)) => kind == wanted,
        Some(Value::Array(kinds)) => kinds.iter().any(|kind| kind == wanted),
        _ => false,
    }
}

/// Whether one branch of the schema accepts a plain string.
pub(super) fn accepts_string(root: &Value, schema: &Value) -> bool {
    branches(root, schema).into_iter().any(|branch| {
        has_type(branch, "string")
            || branch
                .get("enum")
                .and_then(Value::as_array)
                .is_some_and(|values| values.iter().all(Value::is_string))
    })
}

/// The item schema of a schema-defined array, when exactly one branch of the
/// schema is an array with an item schema.
pub(super) fn array_item_schema<'a>(root: &'a Value, schema: &'a Value) -> Option<&'a Value> {
    let mut items = branches(root, schema)
        .into_iter()
        .filter(|branch| branch.get("type").and_then(Value::as_str) == Some("array"))
        .filter_map(|branch| branch.get("items"));
    let item = items.next()?;
    items.next().is_none().then_some(item)
}

/// Whether the schema names an object: each branch that is not `null` is an
/// object, and there is at least one such branch.
pub(super) fn names_object(root: &Value, schema: &Value) -> bool {
    let kept: Vec<&Value> = branches(root, schema)
        .into_iter()
        .filter(|branch| branch.get("type").and_then(Value::as_str) != Some("null"))
        .collect();
    !kept.is_empty()
        && kept
            .iter()
            .all(|branch| branch.get("type").and_then(Value::as_str) == Some("object"))
}

/// The closed set of values of the schema, in schema order, when each branch
/// that is not `null` is an `enum` or a `const`. Otherwise `None`.
fn closed_values(root: &Value, schema: &Value) -> Option<Vec<String>> {
    let mut values: Vec<String> = Vec::new();
    for branch in branches(root, schema) {
        if branch.get("type").and_then(Value::as_str) == Some("null") {
            continue;
        }
        let words: Vec<&Value> = match (branch.get("enum"), branch.get("const")) {
            (Some(Value::Array(words)), _) => words.iter().collect(),
            (None, Some(word)) => vec![word],
            _ => return None,
        };
        for word in words {
            let word = word
                .as_str()
                .map_or_else(|| word.to_string(), str::to_owned);
            if !values.contains(&word) {
                values.push(word);
            }
        }
    }
    (!values.is_empty()).then_some(values)
}

/// Refuses one flag value, and lists the accepted values when the field
/// schema is a closed set.
pub(super) fn invalid_value(root: &Value, schema: &Value, flag: &str) -> anyhow::Error {
    closed_values(root, schema).map_or_else(
        || anyhow::anyhow!("invalid value for --{flag}"),
        |values| {
            anyhow::anyhow!(
                "invalid value for --{flag}; accepted values: {}",
                values.join(", ")
            )
        },
    )
}

#[cfg(test)]
mod tests;
