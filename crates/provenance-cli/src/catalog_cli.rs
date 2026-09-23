//! CLI dispatch over the registered resource catalog.
use crate::invocation::{grammar, GlobalContext};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method};
use clap::{parser::ValueSource, ArgMatches, Command};
use provenance_porcelain::action::Action;
use provenance_store::operations::catalog::{self, Definition, TargetAction};
use serde_json::{json, Map, Value};
use std::{
    collections::BTreeMap,
    io::Read as _,
    net::{Ipv4Addr, SocketAddr},
};

mod address;
mod fields;

pub struct Invocation {
    context: GlobalContext,
    path: String,
    definition: &'static Definition,
    data: Value,
    query: BTreeMap<String, String>,
    headers: HeaderMap,
}

impl Invocation {
    pub fn new(args: &grammar::CatalogArgs, matches: &ArgMatches) -> Self {
        let resolved = address::resolve(&args.collection, &args.address)
            .unwrap_or_else(|error| usage_error(error));
        let (data, query, headers) = input(
            resolved.address.definition,
            matches,
            args.stdin,
            resolved.query,
            &["stdin"],
        )
        .unwrap_or_else(|error| usage_error(error));
        if resolved.address.definition.method == catalog::HttpMethod::Post {
            if let Some(id) = data.get("id").and_then(Value::as_str) {
                provenance_core::ensure_record_id_assignable(id)
                    .unwrap_or_else(|error| usage_error(error));
            }
        }
        Self {
            context: args.common.context(),
            path: resolved.path,
            definition: resolved.address.definition,
            data,
            query,
            headers,
        }
    }
}

pub fn is_collection(word: &str) -> bool {
    !address::registrations(word).is_empty()
}

pub fn command(collection: &str) -> anyhow::Result<Command> {
    let registrations = address::registrations(collection);
    let mut command = grammar::catalog_command();
    let mut help = address::help(collection);
    if collection == "questions" {
        help.push_str("\nA question should be resolvable in one agent session;\notherwise it is fog or needs decomposition.");
    }
    command = command.after_help(help);
    fields::augment(
        command,
        registrations.into_iter().map(|address| address.definition),
        &["repo", "scope", "format", "quiet", "stdin"],
    )
}

pub fn target_command() -> anyhow::Result<Command> {
    let definitions = TargetAction::ALL
        .into_iter()
        .flat_map(catalog::target_definitions)
        .map(|(_, definition)| definition)
        .collect::<Vec<_>>();
    fields::augment_with_overrides(
        grammar::target_command(),
        definitions,
        &[
            "repo",
            "scope",
            "format",
            "quiet",
            "type",
            "view",
            "depth",
            "kind",
            "limit",
            "stdin",
            "status",
            "cursor",
            "body",
            "actor",
            "request-id",
            "expected-version",
            "role",
        ],
        &[
            "status",
            "cursor",
            "body",
            "actor",
            "request-id",
            "expected-version",
            "role",
        ],
    )
}

/// Dispatch one catalog request through the local in-process host.
pub async fn dispatch(invocation: Invocation) -> anyhow::Result<()> {
    let Invocation {
        context,
        path,
        definition,
        data,
        query,
        headers,
    } = invocation;
    let method = match definition.method {
        catalog::HttpMethod::Get => Method::GET,
        catalog::HttpMethod::Post => Method::POST,
        catalog::HttpMethod::Patch => Method::PATCH,
    };
    if matches!(
        path.split('/').nth(1),
        Some("questions" | "contributions" | "synthesis-packets" | "proposals")
    ) {
        warn_if_skills_missing(&context.repo, context.quiet)?;
    }
    let host = local_host(&context)?;
    match host
        .invoke_resource(method, &path, data, query, headers)
        .await
    {
        Ok(value) => crate::output::print_json(&value)?,
        Err(failure) => anyhow::bail!("{}", serde_json::to_string(&failure)?),
    }
    Ok(())
}

pub async fn dispatch_target(
    context: GlobalContext,
    format: Option<provenance_cli::porcelain::OutputFormat>,
    target: String,
    action: Action,
    kind: Option<provenance_core::NodeType>,
    matches: ArgMatches,
) -> anyhow::Result<()> {
    if action == Action::Create {
        provenance_core::ensure_record_id_assignable(&target)
            .unwrap_or_else(|error| usage_error(error));
    }
    let host = local_host(&context)?;
    let route = host
        .target_route(action, &target, kind)
        .await
        .map_err(anyhow::Error::new)?;
    if route.kind == provenance_core::NodeType::Question {
        warn_if_skills_missing(&context.repo, context.quiet)?;
    }
    let stdin = matches.get_flag("stdin");
    let (data, query, headers) = input(
        route.definition,
        &matches,
        stdin,
        None,
        &["record_type", "stdin"],
    )
    .unwrap_or_else(|error| usage_error(error));
    if !query.is_empty() {
        usage_error(anyhow::anyhow!(
            "target actions do not accept query options"
        ));
    }
    let value = host
        .invoke_target(&route, data, headers)
        .await
        .map_err(|failure| anyhow::anyhow!(serde_json::to_string(&failure).unwrap()))?;
    if format == Some(provenance_cli::porcelain::OutputFormat::Json) {
        crate::output::print_json(&value)?;
    } else {
        println!(
            "{}",
            provenance_porcelain::action::render_readable(action, &target, route.kind, &value)
        );
    }
    Ok(())
}

pub fn ensure_only_fields(matches: &ArgMatches, allowed: &[&str]) {
    const COMMON: &[&str] = &[
        "repo",
        "scope",
        "format",
        "quiet",
        "target",
        "action",
        "collection",
        "address",
    ];
    for id in matches.ids() {
        let name = id.as_str();
        if matches.value_source(name) != Some(ValueSource::CommandLine) {
            continue;
        }
        let normalized = name.replace('_', "-");
        if !COMMON.contains(&name)
            && !allowed.contains(&name)
            && !allowed.contains(&normalized.as_str())
        {
            usage_error(anyhow::anyhow!(
                "unsupported option --{}",
                name.replace('_', "-")
            ));
        }
    }
}

pub fn usage_error(error: impl std::fmt::Display) -> ! {
    clap::Error::raw(clap::error::ErrorKind::InvalidValue, error.to_string()).exit()
}

fn local_host(context: &GlobalContext) -> anyhow::Result<provenance_transport::StatementHost> {
    let root = std::fs::canonicalize(&context.repo)?;
    let access = provenance_transport::LocalAccess::new(
        &root,
        "native",
        &context.scope,
        &"0".repeat(64),
        SocketAddr::from((Ipv4Addr::LOCALHOST, 1)),
    )
    .map_err(|failure| anyhow::anyhow!(failure))?;
    Ok(provenance_transport::StatementHost::with_access(
        std::sync::Arc::new(access),
    ))
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

fn input(
    definition: &Definition,
    matches: &ArgMatches,
    stdin: bool,
    query_action: Option<&'static str>,
    extra: &[&str],
) -> anyhow::Result<(Value, BTreeMap<String, String>, HeaderMap)> {
    let declared = fields::declared(definition)?;
    let mut allowed = declared
        .iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();
    allowed.extend(extra.iter().copied());
    ensure_only_fields(matches, &allowed);
    let mut data = Map::new();
    let mut query = BTreeMap::new();
    let mut headers = HeaderMap::new();
    for field in declared {
        let Some(value) = matches
            .try_get_one::<String>(&field.name)
            .ok()
            .flatten()
            .or_else(|| {
                matches
                    .try_get_one::<String>(&field.name.replace('-', "_"))
                    .ok()
                    .flatten()
            })
        else {
            continue;
        };
        match field.source {
            fields::Source::Parameter(parameter) => match parameter.location {
                "query" => {
                    let parsed = catalog::parse_parameter_value(&parameter, value)
                        .map_err(|_| anyhow::anyhow!("invalid value for --{}", field.name))?;
                    let encoded = catalog::serialize_parameter_value(&parameter, &parsed)
                        .map_err(|_| anyhow::anyhow!("invalid value for --{}", field.name))?;
                    query.insert(parameter.name.to_owned(), encoded);
                }
                "header" => {
                    headers.insert(
                        HeaderName::from_bytes(parameter.name.as_bytes())?,
                        HeaderValue::from_str(value)?,
                    );
                }
                _ => anyhow::bail!("unknown catalog parameter location"),
            },
            fields::Source::Body {
                wire_field,
                value_schema,
                wrap_array,
            } => {
                if !wrap_array
                    && matches!(
                        value_schema.get("type").and_then(Value::as_str),
                        Some("array" | "object")
                    )
                {
                    anyhow::bail!("arrays and objects must come from --stdin");
                }
                let request = definition.request_schema().expect("declared body field");
                let parsed = catalog::parse_schema_value_in(request, &value_schema, value)
                    .map_err(|_| anyhow::anyhow!("invalid value for --{}", field.name))?;
                anyhow::ensure!(
                    data.insert(
                        wire_field,
                        if wrap_array { json!([parsed]) } else { parsed }
                    )
                    .is_none(),
                    "body field is supplied by more than one option"
                );
            }
        }
    }
    if let Some(action) = query_action {
        anyhow::ensure!(
            !query.contains_key("query"),
            "query action is supplied twice"
        );
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
