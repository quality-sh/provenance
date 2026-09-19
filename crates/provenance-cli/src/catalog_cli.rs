//! Generated-dialect CLI dispatch over the registered resource catalog.
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method};
use provenance_macros::rule;
use provenance_store::operations::catalog::{self, Definition};
use serde_json::{json, Map, Value};
use std::{
    collections::BTreeMap,
    io::Read as _,
    net::{Ipv4Addr, SocketAddr},
};

mod address;

const COLLECTIONS: &[&str] = &[
    "sources",
    "requirements",
    "resolutions",
    "rules",
    "domains",
    "boundaries",
    "topics",
    "questions",
    "contributions",
    "synthesis-packets",
    "proposals",
    "verification-runs",
    "verification-bindings",
    "statement-checks",
    "authoring-plans",
    "authoring-changes",
    "discussion-containers",
    "messages",
    "assertions",
    "dispositions",
];

/// Dispatches direct graph commands from the live operation catalog.
#[rule("rule_porcelain_regular_graph_work_has_commands")]
pub async fn try_dispatch(arguments: &[String]) -> anyhow::Result<bool> {
    let Some((context, mut words)) = split_global(arguments)? else {
        return Ok(false);
    };
    let collection = words.remove(0);
    if words.as_slice() == ["--help"] {
        print_help(&collection);
        return Ok(true);
    }
    let resolved = address::resolve(&collection, &words)?;
    if resolved.flags.as_slice() == ["--help"] {
        print_help(&collection);
        return Ok(true);
    }
    let definition = resolved.address.definition;
    let method = match definition.method {
        catalog::HttpMethod::Get => Method::GET,
        catalog::HttpMethod::Post => Method::POST,
        catalog::HttpMethod::Patch => Method::PATCH,
    };
    let (data, query, headers) = input(definition, &resolved.flags, resolved.query)?;
    if matches!(
        collection.as_str(),
        "questions" | "contributions" | "synthesis-packets" | "proposals"
    ) {
        warn_if_skills_missing(&context.repo, context.quiet)?;
    }
    let root = std::fs::canonicalize(&context.repo)?;
    let access = provenance_transport::LocalAccess::new(
        &root,
        "native",
        &context.scope,
        &"0".repeat(64),
        SocketAddr::from((Ipv4Addr::LOCALHOST, 1)),
    )
    .map_err(|failure| anyhow::anyhow!(failure))?;
    let host = provenance_transport::StatementHost::with_access(std::sync::Arc::new(access));
    match host
        .invoke_resource(method, &resolved.path, data, query, headers)
        .await
    {
        Ok(value) => crate::output::print_json(&value)?,
        Err(failure) => anyhow::bail!("{}", serde_json::to_string(&failure)?),
    }
    Ok(true)
}

fn print_help(collection: &str) {
    println!("Catalog commands for {collection}:");
    println!("  {collection} list");
    println!("  {collection} create [scalar flags | --stdin]");
    println!("  {collection} <id> [get|update|trace|neighbors|impact|action]");
    if collection == "questions" {
        println!("A question should be resolvable in one agent session;");
        println!("otherwise it is fog or needs decomposition.");
    }
}

fn warn_if_skills_missing(repo: &str, quiet: bool) -> anyhow::Result<()> {
    if quiet {
        return Ok(());
    }
    let status = crate::skills::install_status(std::path::Path::new(repo))?;
    if !status.installed {
        eprintln!(
            "hint: provenance skills are not installed; run `{}` from the repo root",
            status.install_command
        );
    }
    Ok(())
}

struct Context {
    repo: String,
    scope: String,
    quiet: bool,
}

fn split_global(arguments: &[String]) -> anyhow::Result<Option<(Context, Vec<String>)>> {
    if !begins_catalog_command(arguments) {
        return Ok(None);
    }
    let mut repo = ".".to_owned();
    let mut scope = "default".to_owned();
    let mut format = None;
    let mut quiet = false;
    let mut rest = Vec::new();
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--quiet" => {
                quiet = true;
                index += 1;
            }
            "--repo" | "--scope" | "--format" => {
                let value = arguments
                    .get(index + 1)
                    .ok_or_else(|| anyhow::anyhow!("{} requires a value", arguments[index]))?;
                match arguments[index].as_str() {
                    "--repo" => repo.clone_from(value),
                    "--scope" => scope.clone_from(value),
                    _ => format = Some(value.clone()),
                }
                index += 2;
            }
            word if word.starts_with("--") && word != "--stdin" && word != "--help" => {
                rest.push(word.to_owned());
                let value = arguments
                    .get(index + 1)
                    .ok_or_else(|| anyhow::anyhow!("{word} requires a value"))?;
                rest.push(value.clone());
                index += 2;
            }
            word => {
                rest.push(word.to_owned());
                index += 1;
            }
        }
    }
    if rest
        .first()
        .is_none_or(|word| !COLLECTIONS.contains(&word.as_str()))
    {
        return Ok(None);
    }
    anyhow::ensure!(
        format.as_deref().is_none_or(|format| format == "json"),
        "catalog commands support --format json"
    );
    Ok(Some((Context { repo, scope, quiet }, rest)))
}

fn begins_catalog_command(arguments: &[String]) -> bool {
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--quiet" => index += 1,
            "--repo" | "--scope" | "--format" => index += 2,
            word => return COLLECTIONS.contains(&word),
        }
    }
    false
}

#[allow(clippy::too_many_lines)]
fn input(
    definition: &Definition,
    words: &[String],
    query_action: Option<&'static str>,
) -> anyhow::Result<(Value, BTreeMap<String, String>, HeaderMap)> {
    let mut data = Map::new();
    let mut query = BTreeMap::new();
    let mut headers = HeaderMap::new();
    let mut stdin = false;
    let mut index = 0;
    while index < words.len() {
        if words[index] == "--stdin" {
            stdin = true;
            index += 1;
            continue;
        }
        let flag = words[index]
            .strip_prefix("--")
            .ok_or_else(|| anyhow::anyhow!("unexpected argument: {}", words[index]))?;
        let value = words
            .get(index + 1)
            .ok_or_else(|| anyhow::anyhow!("--{flag} requires a value"))?;
        if let Some(parameter) = definition
            .parameters()
            .iter()
            .find(|parameter| cli_name(parameter.name) == flag)
        {
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
                "path" => anyhow::bail!("path identity comes from the command address"),
                _ => anyhow::bail!("unknown catalog parameter location"),
            }
        } else if let Some(request_schema) = definition.request_schema() {
            let field = flag.replace('-', "_");
            let alias = definition
                .registration
                .request
                .argument_aliases
                .iter()
                .find(|alias| alias.argument == field);
            let wire_field = alias.map_or(field.as_str(), |alias| alias.field);
            let field_schema = body_field_schema(request_schema, wire_field)
                .ok_or_else(|| anyhow::anyhow!("unknown body field: --{flag}"))?;
            let value_schema = alias
                .filter(|alias| alias.wrap_array)
                .and_then(|_| field_schema.get("items"))
                .unwrap_or(field_schema);
            if alias.is_none_or(|alias| !alias.wrap_array)
                && matches!(
                    field_schema.get("type").and_then(Value::as_str),
                    Some("array" | "object")
                )
            {
                anyhow::bail!("arrays and objects must come from --stdin");
            }
            let parsed = catalog::parse_schema_value_in(request_schema, value_schema, value)
                .map_err(|_| anyhow::anyhow!("invalid value for --{flag}"))?;
            data.insert(
                wire_field.to_owned(),
                if alias.is_some_and(|alias| alias.wrap_array) {
                    json!([parsed])
                } else {
                    parsed
                },
            );
        } else {
            query.insert(flag.replace('-', "_"), value.clone());
        }
        index += 2;
    }
    if let Some(action) = query_action {
        query.insert("query".into(), action.to_owned());
    }
    if stdin {
        anyhow::ensure!(
            data.is_empty(),
            "--stdin cannot be combined with body field flags"
        );
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text)?;
        data = serde_json::from_str::<Value>(&text)?
            .as_object()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("stdin must contain one JSON object"))?;
    }
    for default in &definition.registration.cli.defaults {
        data.entry(default.field)
            .or_insert_with(|| match default.value {
                catalog::CliDefaultValue::String(value) => json!(value),
                catalog::CliDefaultValue::EmptyArray => json!([]),
            });
    }
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

fn body_field_schema<'a>(request: &'a Value, field: &str) -> Option<&'a Value> {
    let properties = request
        .pointer("/properties/data/properties")?
        .as_object()?;
    properties.get(field)
}

fn cli_name(name: &str) -> String {
    name.to_ascii_lowercase().replace('_', "-")
}
