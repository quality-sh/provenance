//! Generated-dialect CLI dispatch over the registered resource catalog.
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method};
use crate::invocation::GlobalContext;
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

pub struct Invocation {
    context: GlobalContext,
    words: Vec<String>,
}

impl Invocation {
    pub fn new(
        context: GlobalContext,
        format: Option<&str>,
        words: Vec<String>,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            words.first().is_some_and(|word| is_collection(word)),
            "catalog command requires a registered collection"
        );
        anyhow::ensure!(
            format.is_none_or(|format| format == "json"),
            "catalog commands support --format json"
        );
        Ok(Self { context, words })
    }
}

pub fn is_collection(word: &str) -> bool {
    COLLECTIONS.contains(&word)
}

/// Dispatches one parsed graph command from the live operation catalog.
pub async fn dispatch(invocation: Invocation) -> anyhow::Result<()> {
    let Invocation {
        context,
        mut words,
    } = invocation;
    let collection = words.remove(0);
    if words.as_slice() == ["--help"] {
        print_help(&collection);
        return Ok(());
    }
    let resolved = address::resolve(&collection, &words)?;
    if resolved.flags.as_slice() == ["--help"] {
        print_help(&collection);
        return Ok(());
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
    Ok(())
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
