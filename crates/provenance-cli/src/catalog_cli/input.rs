//! Structured catalog CLI input bound from Clap matches.
//!
//! Plain flags carry one scalar or one array item per use; a `--<field>-json`
//! flag carries one whole JSON value; `--stdin` fills only the body fields
//! that no flag assigned. Query parameters keep the canonical parameter
//! encoding that the shared catalog contract defines.

use super::fields::{self, Field, Source};
use crate::catalog_cli::ensure_only_fields;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use clap::ArgMatches;
use provenance_store::operations::catalog::{self, Definition};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;

pub(super) type Parsed = (Value, BTreeMap<String, String>, HeaderMap);

/// One request body field that a flag already assigned.
struct Assignment {
    flag: String,
}

/// One request body field with the item schema that plain flags carry.
struct BodyField<'a> {
    wire_name: &'a str,
    schema: &'a Value,
    item_schema: Option<&'a Value>,
}

/// Bind one catalog request from the parsed command line.
pub(super) fn parse(
    definition: &Definition,
    matches: &ArgMatches,
    stdin: bool,
    query_action: Option<&'static str>,
    extra: &[&str],
) -> anyhow::Result<Parsed> {
    let declared = fields::declared(definition)?;
    let request = definition.request_schema();
    let wire_fields = unique_wire_fields(&declared);
    let mut allowed = declared
        .iter()
        .map(|field| field.name.clone())
        .collect::<Vec<_>>();
    allowed.extend(extra.iter().map(|name| (*name).to_owned()));
    for wire_field in &wire_fields {
        allowed.push(fields::json_flag(wire_field));
    }
    let allowed = allowed.iter().map(String::as_str).collect::<Vec<_>>();
    ensure_only_fields(matches, &allowed);

    let mut data = Map::new();
    let mut assignments = BTreeMap::<String, Assignment>::new();
    let mut query = BTreeMap::new();
    let mut headers = HeaderMap::new();
    for field in ordered_fields(definition, declared) {
        let values = supplied(matches, &field.name);
        if values.is_empty() {
            continue;
        }
        match field.source {
            Source::Parameter(parameter) => {
                bind_parameter(&parameter, &field.name, &values, &mut query, &mut headers)?;
            }
            Source::Body {
                wire_field,
                value_schema,
                wrap_array,
            } => {
                let request = request.expect("declared body field");
                let item_schema = if wrap_array {
                    Some(value_schema.clone())
                } else {
                    array_item_schema(request, &value_schema).cloned()
                };
                let body = BodyField {
                    wire_name: &wire_field,
                    schema: &value_schema,
                    item_schema: item_schema.as_ref(),
                };
                assign_plain(
                    &mut data,
                    &mut assignments,
                    request,
                    &body,
                    &field.name,
                    &values,
                )?;
            }
        }
    }
    if let Some(request) = request {
        bind_json_fields(
            request,
            matches,
            &wire_fields,
            &mut data,
            &mut assignments,
        )?;
    }
    if let Some(action) = query_action {
        anyhow::ensure!(
            !query.contains_key("query"),
            "query action is supplied twice"
        );
        query.insert("query".into(), action.to_owned());
    }
    if stdin {
        merge_stdin(&mut data, &assignments)?;
    }
    apply_defaults(definition, &mut data);
    if definition.parameters().iter().any(|parameter| {
        parameter.location == "header" && parameter.required && parameter.name == "Idempotency-Key"
    }) && !headers.contains_key("Idempotency-Key")
    {
        headers.insert(
            HeaderName::from_static("idempotency-key"),
            HeaderValue::from_str(&uuid::Uuid::new_v4().to_string())?,
        );
    }
    Ok((Value::Object(data), query, headers))
}

/// Alias fields bind before canonical fields, so a competing canonical flag
/// keeps its wire field and the alias is the spelling that fails.
fn ordered_fields(definition: &Definition, declared: Vec<Field>) -> Vec<Field> {
    let aliases = definition
        .registration
        .request
        .argument_aliases
        .iter()
        .map(|alias| alias.argument.replace('_', "-"))
        .collect::<Vec<_>>();
    let (mut ordered, canonical): (Vec<_>, Vec<_>) = declared
        .into_iter()
        .partition(|field| aliases.contains(&field.name));
    ordered.extend(canonical);
    ordered
}

/// One wire field per unique body field, in declaration order.
fn unique_wire_fields(declared: &[Field]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut wire_fields = Vec::new();
    for field in declared {
        if let Source::Body { wire_field, .. } = &field.source {
            if seen.insert(wire_field.clone()) {
                wire_fields.push(wire_field.clone());
            }
        }
    }
    wire_fields
}

fn bind_parameter(
    parameter: &catalog::Parameter,
    flag: &str,
    values: &[String],
    query: &mut BTreeMap<String, String>,
    headers: &mut HeaderMap,
) -> anyhow::Result<()> {
    let [value] = values else {
        anyhow::bail!("--{flag} is assigned more than once");
    };
    match parameter.location {
        "query" => {
            let parsed = catalog::parse_parameter_value(parameter, value)
                .map_err(|_| anyhow::anyhow!("invalid value for --{flag}"))?;
            let encoded = catalog::serialize_parameter_value(parameter, &parsed)
                .map_err(|_| anyhow::anyhow!("invalid value for --{flag}"))?;
            query.insert(parameter.name.to_owned(), encoded);
        }
        "header" => {
            headers.insert(
                HeaderName::from_bytes(parameter.name.as_bytes())?,
                HeaderValue::from_str(value)?,
            );
        }
        _ => anyhow::bail!("unknown catalog parameter location"),
    }
    Ok(())
}

/// Bind plain flag values for one body field: one item per use.
fn assign_plain(
    data: &mut Map<String, Value>,
    assignments: &mut BTreeMap<String, Assignment>,
    request: &Value,
    field: &BodyField<'_>,
    flag: &str,
    values: &[String],
) -> anyhow::Result<()> {
    if let Some(previous) = assignments.get(field.wire_name) {
        anyhow::bail!(
            "--{flag} conflicts with --{} for {}",
            previous.flag,
            field.wire_name
        );
    }
    let repeatable = field.item_schema.is_some();
    anyhow::ensure!(
        repeatable || values.len() == 1,
        "body field {} is assigned more than once",
        field.wire_name
    );
    let mut parsed = Vec::new();
    for raw in values {
        parsed.push(parse_plain_value(request, field, flag, raw)?);
    }
    let value = if repeatable {
        Value::Array(parsed)
    } else {
        parsed.remove(0)
    };
    assignments.insert(
        field.wire_name.to_owned(),
        Assignment {
            flag: flag.to_owned(),
        },
    );
    data.insert(field.wire_name.to_owned(), value);
    Ok(())
}

/// Bind one whole JSON value per body field from its `--<field>-json` flag.
fn bind_json_fields(
    request: &Value,
    matches: &ArgMatches,
    wire_fields: &[String],
    data: &mut Map<String, Value>,
    assignments: &mut BTreeMap<String, Assignment>,
) -> anyhow::Result<()> {
    for wire_field in wire_fields {
        let flag = fields::json_flag(wire_field);
        let values = supplied(matches, &flag);
        if values.is_empty() {
            continue;
        }
        anyhow::ensure!(
            values.len() == 1,
            "--{flag} is assigned more than once"
        );
        if let Some(previous) = assignments.get(wire_field) {
            anyhow::bail!(
                "--{flag} conflicts with --{} for {wire_field}",
                previous.flag
            );
        }
        let schema = fields::wire_field_schema(request, wire_field)
            .ok_or_else(|| anyhow::anyhow!("catalog body field {wire_field} has no schema"))?;
        let parsed = parse_json(request, schema, &flag, &values[0])?;
        assignments.insert(
            wire_field.clone(),
            Assignment { flag },
        );
        data.insert(wire_field.clone(), parsed);
    }
    Ok(())
}

/// Fill only the body fields that no flag assigned from one stdin object.
fn merge_stdin(
    data: &mut Map<String, Value>,
    assignments: &BTreeMap<String, Assignment>,
) -> anyhow::Result<()> {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text)?;
    let object = serde_json::from_str::<Value>(&text)?
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("stdin must contain one JSON object"))?;
    for (field, value) in object {
        if let Some(assignment) = assignments.get(&field) {
            anyhow::bail!("stdin field {field} conflicts with --{}", assignment.flag);
        }
        data.insert(field, value);
    }
    Ok(())
}

fn supplied(matches: &ArgMatches, name: &str) -> Vec<String> {
    let read = |name: &str| {
        matches
            .try_get_many::<String>(name)
            .ok()
            .flatten()
            .map(|values| values.map(String::to_owned).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    let values = read(name);
    if values.is_empty() {
        read(&name.replace('-', "_"))
    } else {
        values
    }
}

/// Parse one plain value against the item schema for array fields, or the
/// field schema otherwise. A schema that admits strings keeps the exact text.
fn parse_plain_value(
    request: &Value,
    field: &BodyField<'_>,
    flag: &str,
    raw: &str,
) -> anyhow::Result<Value> {
    if let Some(item) = field.item_schema {
        if is_structured_json(raw) {
            anyhow::bail!(
                "arrays and objects must come from --stdin or --{}; \
                 --{flag} accepts one item per use",
                fields::json_flag(field.wire_name)
            );
        }
        return parse_typed(request, item, flag, raw);
    }
    if schema_kind(request, field.schema, 0) == Some("object") {
        anyhow::bail!(
            "objects must come from --stdin or --{}",
            fields::json_flag(field.wire_name)
        );
    }
    parse_typed(request, field.schema, flag, raw)
}

fn parse_typed(request: &Value, schema: &Value, flag: &str, raw: &str) -> anyhow::Result<Value> {
    if accepts_string(request, schema, 0) {
        let value = Value::String(raw.to_owned());
        anyhow::ensure!(validates(request, schema, &value)?, "invalid value for --{flag}");
        return Ok(value);
    }
    let value = catalog::parse_schema_value_in(request, schema, raw)
        .map_err(|_| anyhow::anyhow!("invalid value for --{flag}"))?;
    anyhow::ensure!(validates(request, schema, &value)?, "invalid value for --{flag}");
    Ok(value)
}

fn parse_json(request: &Value, schema: &Value, flag: &str, raw: &str) -> anyhow::Result<Value> {
    let value = serde_json::from_str(raw)
        .map_err(|_| anyhow::anyhow!("invalid JSON value for --{flag}"))?;
    anyhow::ensure!(validates(request, schema, &value)?, "invalid value for --{flag}");
    Ok(value)
}

fn is_structured_json(raw: &str) -> bool {
    serde_json::from_str::<Value>(raw)
        .is_ok_and(|value| matches!(value, Value::Array(_) | Value::Object(_)))
}

/// Validate one value against a field schema with the request definitions.
fn validates(root: &Value, schema: &Value, value: &Value) -> anyhow::Result<bool> {
    let mut standalone = schema.clone();
    let object = standalone
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("catalog field schema is not an object"))?;
    if let Some(definitions) = root.get("$defs") {
        object.insert("$defs".into(), definitions.clone());
    }
    let validator = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(&standalone)
        .map_err(|error| anyhow::anyhow!("invalid catalog field schema: {error}"))?;
    Ok(validator.is_valid(value))
}

/// Whether one schema admits a plain string, following references and unions.
fn accepts_string(root: &Value, schema: &Value, depth: usize) -> bool {
    if depth > 64 {
        return false;
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return resolve(root, reference)
            .is_some_and(|schema| accepts_string(root, schema, depth + 1));
    }
    if let Some(variants) = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)
    {
        return variants
            .iter()
            .any(|schema| accepts_string(root, schema, depth + 1));
    }
    schema.get("type").is_some_and(|kind| match kind {
        Value::String(kind) => kind == "string",
        Value::Array(kinds) => kinds.iter().any(|kind| kind == "string"),
        _ => false,
    }) || schema
        .get("enum")
        .and_then(Value::as_array)
        .is_some_and(|values| values.iter().all(Value::is_string))
}

/// The item schema of a schema-defined array, following references and unions
/// that admit exactly one array shape.
fn array_item_schema<'a>(root: &'a Value, schema: &'a Value) -> Option<&'a Value> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return resolve(root, reference).and_then(|schema| array_item_schema(root, schema));
    }
    if schema.get("type") == Some(&json!("array")) {
        return schema.get("items");
    }
    let variants = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)?;
    let mut items = variants
        .iter()
        .filter_map(|schema| array_item_schema(root, schema));
    let item = items.next()?;
    items.next().is_none().then_some(item)
}

/// Whether one schema names an object, following references and unions.
fn schema_kind(root: &Value, schema: &Value, depth: usize) -> Option<&'static str> {
    if depth > 64 {
        return None;
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return resolve(root, reference).and_then(|schema| schema_kind(root, schema, depth + 1));
    }
    if let Some(variants) = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)
    {
        let mut kind = None;
        for variant in variants {
            if variant.get("type") == Some(&json!("null")) {
                continue;
            }
            let current = schema_kind(root, variant, depth + 1)?;
            if kind.is_some_and(|previous| previous != current) {
                return None;
            }
            kind = Some(current);
        }
        return kind;
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("object") => Some("object"),
        _ => None,
    }
}

fn resolve<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    let name = reference.strip_prefix("#/$defs/")?;
    root.get("$defs")?.get(name)
}

fn apply_defaults(definition: &Definition, data: &mut Map<String, Value>) {
    for default in &definition.registration.cli.defaults {
        data.entry(default.field)
            .or_insert_with(|| match default.value {
                catalog::CliDefaultValue::String(value) => json!(value),
                catalog::CliDefaultValue::EmptyArray => json!([]),
            });
    }
}
