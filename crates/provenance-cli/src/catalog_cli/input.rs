use axum::http::{HeaderMap, HeaderName, HeaderValue};
use provenance_store::operations::catalog::{self, Definition};
use serde_json::{json, Map, Value};
use std::{collections::BTreeMap, io::Read as _};

pub(super) type Parsed = (Value, BTreeMap<String, String>, HeaderMap);

struct BodyField<'a> {
    wire_name: String,
    schema: &'a Value,
    item_schema: Option<&'a Value>,
}

struct Assignment {
    flag: String,
    repeatable: bool,
}

pub(super) fn parse(
    definition: &Definition,
    words: &[String],
    query_action: Option<&'static str>,
) -> anyhow::Result<Parsed> {
    let mut data = Map::new();
    let mut assignments = BTreeMap::<String, Assignment>::new();
    let mut query = BTreeMap::new();
    let mut headers = HeaderMap::new();
    let mut stdin_count = 0;
    let mut index = 0;
    while index < words.len() {
        if words[index] == "--stdin" {
            stdin_count += 1;
            anyhow::ensure!(stdin_count == 1, "--stdin may be specified only once");
            index += 1;
            continue;
        }
        let flag = words[index]
            .strip_prefix("--")
            .ok_or_else(|| anyhow::anyhow!("unexpected argument: {}", words[index]))?;
        let raw = words
            .get(index + 1)
            .ok_or_else(|| anyhow::anyhow!("--{flag} requires a value"))?;
        if let Some(parameter) = definition
            .parameters()
            .iter()
            .find(|parameter| cli_name(parameter.name) == flag)
        {
            insert_parameter(parameter, flag, raw, &mut query, &mut headers)?;
        } else if let Some(request_schema) = definition.request_schema() {
            if let Some(field_name) = flag.strip_suffix("-json") {
                let field = structured_field(request_schema, field_name, flag)?;
                let parsed = parse_json(request_schema, field.schema, raw, flag)?;
                assign_complete(&mut data, &mut assignments, &field, flag, parsed)?;
            } else {
                let field = plain_field(definition, request_schema, flag)?;
                let value_schema = field.item_schema.unwrap_or(field.schema);
                let parsed = match parse_plain(request_schema, value_schema, raw, flag) {
                    Ok(parsed) => parsed,
                    Err(_) if field.item_schema.is_some() && is_structured_json(raw) => {
                        let json_flag = field.wire_name.replace('_', "-");
                        anyhow::bail!(
                            "arrays and objects must come from --stdin or --{json_flag}-json; \
                             --{flag} accepts one item per use"
                        );
                    }
                    Err(error) => return Err(error),
                };
                assign_plain(&mut data, &mut assignments, &field, flag, parsed)?;
            }
        } else {
            insert_query(&mut query, &flag.replace('-', "_"), raw, flag)?;
        }
        index += 2;
    }
    if let Some(action) = query_action {
        insert_query(&mut query, "query", action, "query action")?;
    }
    if stdin_count == 1 {
        merge_stdin(&mut data, &assignments)?;
    }
    apply_defaults(definition, &mut data);
    apply_idempotency_key(definition, &mut headers)?;
    Ok((Value::Object(data), query, headers))
}

fn insert_parameter(
    parameter: &catalog::Parameter,
    flag: &str,
    raw: &str,
    query: &mut BTreeMap<String, String>,
    headers: &mut HeaderMap,
) -> anyhow::Result<()> {
    match parameter.location {
        "query" => insert_query(query, parameter.name, raw, flag),
        "header" => {
            let name = HeaderName::from_bytes(parameter.name.as_bytes())?;
            anyhow::ensure!(
                !headers.contains_key(&name),
                "--{flag} is assigned more than once"
            );
            headers.insert(name, HeaderValue::from_str(raw)?);
            Ok(())
        }
        "path" => anyhow::bail!("path identity comes from the command address"),
        _ => anyhow::bail!("unknown catalog parameter location"),
    }
}

fn insert_query(
    query: &mut BTreeMap<String, String>,
    name: &str,
    value: &str,
    source: &str,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        !query.contains_key(name),
        "query parameter {name} conflicts with {source}"
    );
    query.insert(name.to_owned(), value.to_owned());
    Ok(())
}

fn structured_field<'a>(
    request: &'a Value,
    field_name: &str,
    flag: &str,
) -> anyhow::Result<BodyField<'a>> {
    let wire_name = field_name.replace('-', "_");
    let schema = body_field_schema(request, &wire_name)
        .ok_or_else(|| anyhow::anyhow!("unknown body field: --{flag}"))?;
    Ok(BodyField {
        wire_name,
        schema,
        item_schema: None,
    })
}

fn plain_field<'a>(
    definition: &Definition,
    request: &'a Value,
    flag: &str,
) -> anyhow::Result<BodyField<'a>> {
    let argument = flag.replace('-', "_");
    let alias = definition
        .registration
        .request
        .argument_aliases
        .iter()
        .find(|alias| alias.argument == argument);
    let wire_name = alias.map_or_else(|| argument.clone(), |alias| alias.field.to_owned());
    let schema = body_field_schema(request, &wire_name)
        .ok_or_else(|| anyhow::anyhow!("unknown body field: --{flag}"))?;
    let item_schema = array_item_schema(request, schema);
    anyhow::ensure!(
        alias.is_none_or(|alias| !alias.wrap_array || item_schema.is_some()),
        "invalid CLI alias registration for --{flag}"
    );
    Ok(BodyField {
        wire_name,
        schema,
        item_schema,
    })
}

fn assign_plain(
    data: &mut Map<String, Value>,
    assignments: &mut BTreeMap<String, Assignment>,
    field: &BodyField<'_>,
    flag: &str,
    parsed: Value,
) -> anyhow::Result<()> {
    if let Some(previous) = assignments.get(&field.wire_name) {
        if previous.flag != flag {
            anyhow::bail!(
                "--{flag} conflicts with --{} for {}",
                previous.flag,
                field.wire_name
            );
        }
        anyhow::ensure!(
            previous.repeatable && field.item_schema.is_some(),
            "body field {} is assigned more than once",
            field.wire_name
        );
        data.get_mut(&field.wire_name)
            .and_then(Value::as_array_mut)
            .expect("repeatable body assignment is an array")
            .push(parsed);
        return Ok(());
    }
    let repeatable = field.item_schema.is_some();
    assignments.insert(
        field.wire_name.clone(),
        Assignment {
            flag: flag.to_owned(),
            repeatable,
        },
    );
    data.insert(
        field.wire_name.clone(),
        if repeatable { json!([parsed]) } else { parsed },
    );
    Ok(())
}

fn assign_complete(
    data: &mut Map<String, Value>,
    assignments: &mut BTreeMap<String, Assignment>,
    field: &BodyField<'_>,
    flag: &str,
    parsed: Value,
) -> anyhow::Result<()> {
    if let Some(previous) = assignments.get(&field.wire_name) {
        anyhow::bail!(
            "--{flag} conflicts with --{} for {}",
            previous.flag,
            field.wire_name
        );
    }
    assignments.insert(
        field.wire_name.clone(),
        Assignment {
            flag: flag.to_owned(),
            repeatable: false,
        },
    );
    data.insert(field.wire_name.clone(), parsed);
    Ok(())
}

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

fn parse_plain(root: &Value, schema: &Value, raw: &str, flag: &str) -> anyhow::Result<Value> {
    if accepts_string(root, schema, 0) {
        let value = Value::String(raw.to_owned());
        anyhow::ensure!(
            validates(root, schema, &value)?,
            "invalid value for --{flag}"
        );
        return Ok(value);
    }
    catalog::parse_schema_value_in(root, schema, raw)
        .map_err(|_| anyhow::anyhow!("invalid value for --{flag}"))
}

fn parse_json(root: &Value, schema: &Value, raw: &str, flag: &str) -> anyhow::Result<Value> {
    let value = serde_json::from_str(raw)
        .map_err(|_| anyhow::anyhow!("invalid JSON value for --{flag}"))?;
    anyhow::ensure!(
        validates(root, schema, &value)?,
        "invalid value for --{flag}"
    );
    Ok(value)
}

fn is_structured_json(raw: &str) -> bool {
    serde_json::from_str::<Value>(raw)
        .is_ok_and(|value| matches!(value, Value::Array(_) | Value::Object(_)))
}

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

fn resolve<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    let name = reference.strip_prefix("#/$defs/")?;
    root.get("$defs")?.get(name)
}

fn body_field_schema<'a>(request: &'a Value, field: &str) -> Option<&'a Value> {
    request
        .pointer("/properties/data/properties")?
        .as_object()?
        .get(field)
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

fn apply_idempotency_key(definition: &Definition, headers: &mut HeaderMap) -> anyhow::Result<()> {
    if definition.parameters().iter().any(|parameter| {
        parameter.location == "header" && parameter.required && parameter.name == "Idempotency-Key"
    }) && !headers.contains_key("Idempotency-Key")
    {
        headers.insert(
            HeaderName::from_static("idempotency-key"),
            HeaderValue::from_str(&uuid::Uuid::new_v4().to_string())?,
        );
    }
    Ok(())
}

fn cli_name(name: &str) -> String {
    name.to_ascii_lowercase().replace('_', "-")
}
