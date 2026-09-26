//! One schema-defined text encoding for HTTP, MCP, CLI, and generated clients.

use super::schema::Parameter;
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug)]
pub struct ParseValueError;

pub fn parse_parameter_value(parameter: &Parameter, raw: &str) -> Result<Value, ParseValueError> {
    parse_schema_value(&parameter.schema, raw)
}

pub fn serialize_parameter_value(
    parameter: &Parameter,
    value: &Value,
) -> Result<String, ParseValueError> {
    let raw = match value {
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Null => "null".to_owned(),
        Value::Array(values) => values
            .iter()
            .map(|value| match value {
                Value::String(value) => Ok(value.clone()),
                Value::Bool(value) => Ok(value.to_string()),
                Value::Number(value) => Ok(value.to_string()),
                Value::Null => Ok("null".to_owned()),
                Value::Array(_) | Value::Object(_) => Err(ParseValueError),
            })
            .collect::<Result<Vec<_>, _>>()?
            .join(","),
        Value::Object(_) => return Err(ParseValueError),
    };
    (parse_parameter_value(parameter, &raw)? == *value)
        .then_some(raw)
        .ok_or(ParseValueError)
}

pub fn parse_schema_value(schema: &Value, raw: &str) -> Result<Value, ParseValueError> {
    parse_schema_value_in(schema, schema, raw)
}

pub fn parse_schema_value_in(
    root: &Value,
    schema: &Value,
    raw: &str,
) -> Result<Value, ParseValueError> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let name = reference.strip_prefix("#/$defs/").ok_or(ParseValueError)?;
        let resolved = root
            .get("$defs")
            .and_then(|defs| defs.get(name))
            .ok_or(ParseValueError)?;
        return parse_schema_value_in(root, resolved, raw);
    }
    if let Some(variants) = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)
    {
        if raw == "null"
            && variants
                .iter()
                .any(|variant| variant.get("type") == Some(&json!("null")))
        {
            return Ok(Value::Null);
        }
        return variants
            .iter()
            .filter(|variant| variant.get("type") != Some(&json!("null")))
            .find_map(|variant| parse_schema_value_in(root, variant, raw).ok())
            .ok_or(ParseValueError);
    }
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        let value = Value::String(raw.to_owned());
        return values
            .contains(&value)
            .then_some(value)
            .ok_or(ParseValueError);
    }
    if let Some(types) = schema.get("type").and_then(Value::as_array) {
        if raw == "null" && types.iter().any(|kind| kind == "null") {
            return Ok(Value::Null);
        }
        return types
            .iter()
            .filter_map(Value::as_str)
            .filter(|kind| *kind != "null")
            .find_map(|kind| {
                let mut variant = schema.clone();
                variant["type"] = json!(kind);
                parse_schema_value_in(root, &variant, raw).ok()
            })
            .ok_or(ParseValueError);
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("string") => Ok(Value::String(raw.to_owned())),
        Some("boolean") => match raw {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err(ParseValueError),
        },
        Some("integer") => {
            let value = raw.parse::<i64>().map_err(|_| ParseValueError)?;
            let minimum = schema.get("minimum").and_then(Value::as_i64);
            let maximum = schema.get("maximum").and_then(Value::as_i64);
            if minimum.is_some_and(|minimum| value < minimum)
                || maximum.is_some_and(|maximum| value > maximum)
            {
                return Err(ParseValueError);
            }
            Ok(Value::from(value))
        }
        Some("number") => {
            let value = raw.parse::<f64>().map_err(|_| ParseValueError)?;
            serde_json::Number::from_f64(value)
                .map(Value::Number)
                .ok_or(ParseValueError)
        }
        Some("array") => {
            let items = schema.get("items").ok_or(ParseValueError)?;
            if raw.is_empty() {
                return Ok(Value::Array(Vec::new()));
            }
            raw.split(',')
                .map(|item| parse_schema_value_in(root, items, item))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array)
        }
        Some("null") if raw == "null" => Ok(Value::Null),
        _ => Err(ParseValueError),
    }
}
