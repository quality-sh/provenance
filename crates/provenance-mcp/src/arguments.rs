//! The check of tool arguments against the input schema that tools/list gives.
//!
//! A refusal names the field, gives a reason (`required`, `unknown_field`, or
//! `invalid_value`), and its message says what the field takes: the allowed values,
//! the range, the pattern, or the type.

use jsonschema::{error::ValidationErrorKind, paths::LocationSegment, ValidationError};
use serde_json::{json, Map, Value};

/// The input schema of one listed tool, compiled once when the server starts.
pub struct InputCheck {
    schema: Value,
    validator: jsonschema::Validator,
}

impl InputCheck {
    /// Compile one input schema as JSON Schema 2020-12. Formats are not checked. A
    /// reference to a schema outside this document is refused, not fetched.
    pub fn new(schema: Value) -> Result<Self, ValidationError<'static>> {
        let validator = jsonschema::draft202012::options().build(&schema)?;
        Ok(Self { schema, validator })
    }

    /// The error object for the first argument that the input schema of `tool` refuses.
    pub fn refusal(&self, tool: &str, arguments: &Map<String, Value>) -> Option<Value> {
        let instance = Value::Object(arguments.clone());
        let error = self.validator.iter_errors(&instance).next()?.to_owned();
        Some(describe(tool, &self.schema, innermost(&error)))
    }
}

/// The error inside an `anyOf` or `oneOf` that tells the most: the error of the branch
/// that went deeper into the value, or of the one branch that is not a type mismatch
/// (the other branch of an optional field accepts only null).
fn innermost<'e>(error: &'e ValidationError<'static>) -> &'e ValidationError<'static> {
    let (ValidationErrorKind::AnyOf { context } | ValidationErrorKind::OneOfNotValid { context }) =
        error.kind()
    else {
        return error;
    };
    let firsts = context.iter().filter_map(|branch| branch.first());
    let depth = |candidate: &ValidationError<'_>| candidate.instance_path().iter().count();
    let here = depth(error);
    if let Some(deeper) = firsts.clone().find(|candidate| depth(candidate) > here) {
        return innermost(deeper);
    }
    let matches = firsts
        .filter(|candidate| !matches!(candidate.kind(), ValidationErrorKind::Type { .. }))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [only] => innermost(only),
        _ => error,
    }
}

fn describe(tool: &str, root: &Value, error: &ValidationError<'_>) -> Value {
    let path = field_name(error);
    let schema = keyword_owner(root, error);
    let (field, reason, message) = match error.kind() {
        ValidationErrorKind::AdditionalProperties { unexpected }
        | ValidationErrorKind::UnevaluatedProperties { unexpected } => {
            let name = unexpected.first().cloned().unwrap_or_default();
            let owner = if path.is_empty() { tool } else { &path };
            let takes = schema
                .and_then(|schema| schema["properties"].as_object())
                .map(|properties| properties.keys().cloned().collect::<Vec<_>>().join(", "))
                .unwrap_or_default();
            (
                joined(&path, &name),
                "unknown_field",
                format!("{owner} does not take {name}; it takes: {takes}"),
            )
        }
        ValidationErrorKind::Required { property } => {
            let name = property.as_str().unwrap_or_default();
            let owner = if path.is_empty() { tool } else { &path };
            (
                joined(&path, name),
                "required",
                format!("{owner} needs {name}"),
            )
        }
        kind => {
            let expected = schema
                .and_then(|schema| expectation(root, schema, kind))
                .unwrap_or_else(|| error.to_string());
            (
                path.clone(),
                "invalid_value",
                format!("{path} must be {expected}"),
            )
        }
    };
    json!({"kind": "invalid_input", "field": field, "reason": reason, "message": message})
}

/// The field path of the value that the error is about: `parent.node_type`, `node_types[0]`.
fn field_name(error: &ValidationError<'_>) -> String {
    let mut name = String::new();
    for segment in error.instance_path() {
        match segment {
            LocationSegment::Property(property) => name = joined(&name, &property),
            LocationSegment::Index(index) => name = format!("{name}[{index}]"),
        }
    }
    name
}

fn joined(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_owned()
    } else {
        format!("{path}.{name}")
    }
}

/// The schema object that holds the keyword which refused the value.
fn keyword_owner<'s>(root: &'s Value, error: &ValidationError<'_>) -> Option<&'s Value> {
    let pointer = error.schema_path().as_str();
    let owner = pointer.rsplit_once('/').map_or("", |(owner, _)| owner);
    let mut schema = root;
    for segment in owner.split('/').skip(1) {
        let segment = segment.replace("~1", "/").replace("~0", "~");
        schema = if segment == "$ref" {
            resolved(root, schema)
        } else {
            match schema {
                Value::Array(values) => values.get(segment.parse::<usize>().ok()?)?,
                other => other.get(&segment)?,
            }
        };
    }
    Some(schema)
}

/// What the field takes, in words, for the keyword that refused the value.
fn expectation(root: &Value, schema: &Value, kind: &ValidationErrorKind) -> Option<String> {
    let schema = resolved(root, schema);
    match kind {
        ValidationErrorKind::Minimum { .. }
        | ValidationErrorKind::Maximum { .. }
        | ValidationErrorKind::ExclusiveMinimum { .. }
        | ValidationErrorKind::ExclusiveMaximum { .. } => range(schema),
        ValidationErrorKind::Pattern { pattern } => {
            Some(format!("text that matches the pattern {pattern}"))
        }
        ValidationErrorKind::MinLength { limit } => {
            Some(format!("text of at least {limit} characters"))
        }
        ValidationErrorKind::MaxLength { limit } => {
            Some(format!("text of at most {limit} characters"))
        }
        ValidationErrorKind::Type { .. } => values(root, schema)
            .map(|values| format!("one of: {values}"))
            .or_else(|| type_name(schema)),
        _ => values(root, schema).map(|values| format!("one of: {values}")),
    }
}

/// The schema that a `$ref` names, after each reference in turn.
fn resolved<'s>(root: &'s Value, mut schema: &'s Value) -> &'s Value {
    while let Some(target) = schema["$ref"]
        .as_str()
        .and_then(|reference| reference.strip_prefix('#'))
        .and_then(|pointer| root.pointer(pointer))
    {
        schema = target;
    }
    schema
}

/// The allowed values of an enum, a const, or a choice of them; a null choice is left out.
fn values(root: &Value, schema: &Value) -> Option<String> {
    let schema = resolved(root, schema);
    let words = |values: &[Value]| {
        values
            .iter()
            .filter(|value| !value.is_null())
            .map(|value| {
                value
                    .as_str()
                    .map_or_else(|| value.to_string(), str::to_owned)
            })
            .collect::<Vec<_>>()
    };
    if let Some(Value::Array(values)) = schema.get("enum") {
        return Some(words(values).join(", "));
    }
    if let Some(value) = schema.get("const") {
        return Some(words(std::slice::from_ref(value)).join(", "));
    }
    let branches = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))?
        .as_array()?
        .iter()
        .filter(|branch| **branch != json!({"type": "null"}))
        .map(|branch| values(root, branch))
        .collect::<Option<Vec<_>>>()?;
    Some(branches.join(", "))
}

fn range(schema: &Value) -> Option<String> {
    let minimum = schema
        .get("minimum")
        .or_else(|| schema.get("exclusiveMinimum"));
    let maximum = schema
        .get("maximum")
        .or_else(|| schema.get("exclusiveMaximum"));
    match (minimum, maximum) {
        (Some(minimum), Some(maximum)) => Some(format!("from {minimum} to {maximum}")),
        (Some(minimum), None) => Some(format!("at least {minimum}")),
        (None, Some(maximum)) => Some(format!("at most {maximum}")),
        (None, None) => None,
    }
}

fn type_name(schema: &Value) -> Option<String> {
    let names = match &schema["type"] {
        Value::String(name) => vec![name.as_str()],
        Value::Array(names) => names
            .iter()
            .filter_map(Value::as_str)
            .filter(|name| *name != "null")
            .collect(),
        _ => return None,
    };
    let names = names
        .into_iter()
        .map(|name| match name {
            "object" | "array" | "integer" => format!("an {name}"),
            other => format!("a {other}"),
        })
        .collect::<Vec<_>>();
    (!names.is_empty()).then(|| names.join(" or "))
}

#[cfg(test)]
mod tests {
    use super::InputCheck;
    use provenance_macros::verifies;
    use std::io::{Read, Write};

    /// A read tool reads only the local graph: an input schema with a reference to
    /// another document does not compile, and the check opens no connection to fetch it.
    #[test]
    #[verifies("rule_mcp_tools_read_or_write", examples)]
    fn a_reference_to_another_document_is_not_fetched() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (connected, connections) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                connected.send(()).unwrap();
                let mut request = [0; 4096];
                let _ = stream.read(&mut request);
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\
                      content-length: 2\r\nconnection: close\r\n\r\n{}",
                );
            }
        });
        let schema = serde_json::json!({
            "type": "object",
            "properties": {"field": {"$ref": format!("http://{address}/schema.json")}},
        });
        let compiled = InputCheck::new(schema);
        assert!(compiled.is_err(), "the remote reference compiled");
        assert!(
            connections.try_recv().is_err(),
            "the check opened a connection"
        );
    }
}
