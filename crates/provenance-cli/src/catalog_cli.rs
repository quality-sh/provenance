//! Generated-dialect CLI dispatch over the registered resource catalog.
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method};
use provenance_store::operations::catalog::{self, Definition};
use serde_json::{json, Map, Value};
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    io::Read as _,
    net::{Ipv4Addr, SocketAddr},
};

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

pub async fn try_dispatch(arguments: &[String]) -> anyhow::Result<bool> {
    let Some((context, mut words)) = split_global(arguments)? else {
        return Ok(false);
    };
    let collection = words.remove(0);
    if words.contains(&"--help".to_owned()) {
        print_help(&collection);
        return Ok(true);
    }
    let (definition, path, route_words) = resolve(&collection, &words)?;
    let method = match definition.method {
        catalog::HttpMethod::Get => Method::GET,
        catalog::HttpMethod::Post => Method::POST,
        catalog::HttpMethod::Patch => Method::PATCH,
    };
    let (data, query, headers) = input(&definition, &route_words)?;
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
        .invoke_resource(method, &path, data, query, headers)
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

fn resolve(
    collection: &str,
    words: &[String],
) -> anyhow::Result<(Definition, String, Vec<String>)> {
    let address_len = words
        .iter()
        .position(|word| word.starts_with("--"))
        .unwrap_or(words.len());
    let address = &words[..address_len];
    let mut matches = catalog::definitions()
        .into_iter()
        .filter(|definition| definition.path.split('/').nth(1) == Some(collection))
        .flat_map(|definition| candidates(&definition, address))
        .collect::<Vec<_>>();
    matches.sort_by_key(|candidate| (candidate.score, candidate.consumed));
    let candidate = matches.pop().ok_or_else(|| {
        anyhow::anyhow!(
            "the catalog does not declare the {collection} command: {}",
            address.join(" ")
        )
    })?;
    Ok((
        candidate.definition,
        candidate.path,
        words[candidate.consumed..].to_vec(),
    ))
}

struct Candidate {
    definition: Definition,
    path: String,
    consumed: usize,
    score: usize,
}

fn candidates(definition: &Definition, words: &[String]) -> Vec<Candidate> {
    let route = definition
        .path
        .split('/')
        .skip(2)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let mut patterns: Vec<(Vec<&str>, Vec<&str>)> = Vec::new();
    if route.is_empty() {
        match definition.method {
            catalog::HttpMethod::Get => patterns.push((vec!["list"], Vec::new())),
            catalog::HttpMethod::Post => patterns.push((vec!["create"], Vec::new())),
            catalog::HttpMethod::Patch => {}
        }
        if definition.method == catalog::HttpMethod::Get
            && words.first().is_some_and(|word| {
                definition
                    .registration
                    .queries
                    .iter()
                    .any(|query| query.name == word)
            })
        {
            patterns.push((Vec::new(), Vec::new()));
        }
    } else {
        match definition.method {
            catalog::HttpMethod::Get => {
                patterns.extend([
                    (Vec::new(), Vec::new()),
                    (Vec::new(), vec!["get"]),
                    (Vec::new(), vec!["list"]),
                ]);
                if route.len() == 1 && route[0].starts_with('{') {
                    patterns.push((vec!["get"], Vec::new()));
                }
            }
            catalog::HttpMethod::Patch => {
                patterns.push((Vec::new(), vec!["update"]));
                if route.len() == 1 && route[0].starts_with('{') {
                    patterns.push((vec!["update"], Vec::new()));
                }
            }
            catalog::HttpMethod::Post => {
                patterns.push((Vec::new(), Vec::new()));
                patterns.push((Vec::new(), vec!["create"]));
            }
        }
    }
    patterns
        .into_iter()
        .filter_map(|(prefix, suffix)| match_candidate(definition, &route, words, &prefix, &suffix))
        .collect()
}

fn match_candidate(
    definition: &Definition,
    route: &[&str],
    words: &[String],
    prefix: &[&str],
    suffix: &[&str],
) -> Option<Candidate> {
    let consumed = prefix.len() + route.len() + suffix.len();
    if words.len() < consumed
        || !prefix
            .iter()
            .zip(words)
            .all(|(expected, actual)| expected == actual)
        || !suffix
            .iter()
            .zip(&words[prefix.len() + route.len()..])
            .all(|(expected, actual)| expected == actual)
    {
        return None;
    }
    let route_words = &words[prefix.len()..prefix.len() + route.len()];
    if !route.iter().zip(route_words).all(|(expected, actual)| {
        expected.starts_with('{') && expected.ends_with('}') || expected == actual
    }) {
        return None;
    }
    let mut path = String::new();
    write!(path, "/{}", definition.path.split('/').nth(1)?).ok()?;
    for (expected, actual) in route.iter().zip(route_words) {
        write!(
            path,
            "/{}",
            if expected.starts_with('{') {
                actual.as_str()
            } else {
                *expected
            }
        )
        .ok()?;
    }
    let literals = route.iter().filter(|part| !part.starts_with('{')).count();
    Some(Candidate {
        definition: definition.clone(),
        path,
        consumed,
        score: literals * 4 + prefix.len() + suffix.len(),
    })
}

#[allow(clippy::too_many_lines)]
fn input(
    definition: &Definition,
    words: &[String],
) -> anyhow::Result<(Value, BTreeMap<String, String>, HeaderMap)> {
    let mut data = Map::new();
    let mut query = BTreeMap::new();
    let mut headers = HeaderMap::new();
    let mut stdin = false;
    let mut index = 0;
    let query_action = words
        .first()
        .filter(|word| {
            definition
                .registration
                .queries
                .iter()
                .any(|route| route.name == word.as_str())
        })
        .cloned();
    if query_action.is_some() {
        index += 1;
    }
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
            .parameters
            .iter()
            .find(|parameter| cli_name(parameter.name) == flag)
        {
            match parameter.location {
                "query" => {
                    query.insert(parameter.name.to_owned(), value.clone());
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
        } else if let Some(request_schema) = &definition.request_schema {
            let field = flag.replace('-', "_");
            let wire_field = definition
                .registration
                .request
                .argument_aliases
                .iter()
                .find(|alias| alias.argument == field)
                .map_or(field.as_str(), |alias| alias.field);
            let schema = body_field_schema(request_schema, wire_field)
                .ok_or_else(|| anyhow::anyhow!("unknown body field: --{flag}"))?;
            if value.starts_with('[') || value.starts_with('{') {
                anyhow::bail!("arrays and objects must come from --stdin");
            }
            let parsed = catalog::parse_schema_value_in(request_schema, schema, value)
                .map_err(|_| anyhow::anyhow!("invalid value for --{flag}"))?;
            data.insert(wire_field.to_owned(), parsed);
        } else {
            query.insert(flag.replace('-', "_"), value.clone());
        }
        index += 2;
    }
    if let Some(action) = query_action {
        query.insert("query".into(), action);
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
    if definition.name == "create-requirement" {
        data.entry("actor").or_insert_with(|| json!("cli"));
        data.entry("status").or_insert_with(|| json!("active"));
        data.entry("depends_on").or_insert_with(|| json!([]));
        data.entry("supersedes").or_insert_with(|| json!([]));
    }
    if definition.name == "update-requirement" {
        data.entry("actor").or_insert_with(|| json!("cli"));
        data.entry("clear_fields").or_insert_with(|| json!([]));
    }
    if definition.name == "create-source" {
        data.entry("source_type").or_insert_with(|| json!("policy"));
        data.entry("supersedes").or_insert_with(|| json!([]));
    }
    if definition.name == "create-rule" {
        move_scalar_to_singleton(&mut data, "requirement_id", "requirement_ids");
        move_scalar_to_singleton(&mut data, "resolution_id", "resolution_ids");
        data.entry("requirement_ids").or_insert_with(|| json!([]));
        data.entry("resolution_ids").or_insert_with(|| json!([]));
        data.entry("status").or_insert_with(|| json!("active"));
        data.entry("severity").or_insert_with(|| json!("high"));
    }
    if definition.name == "create-resolution" {
        move_scalar_to_singleton(&mut data, "requirement_id", "requirement_ids");
        data.entry("requirement_ids").or_insert_with(|| json!([]));
        data.entry("supersedes").or_insert_with(|| json!([]));
        data.entry("inputs").or_insert_with(|| json!([]));
        data.entry("status").or_insert_with(|| json!("proposed"));
    }
    if definition.name == "create-topic" {
        data.entry("status").or_insert_with(|| json!("open"));
        data.entry("links").or_insert_with(|| json!([]));
    }
    if definition.name == "create-question" {
        data.entry("status").or_insert_with(|| json!("open"));
        data.entry("links").or_insert_with(|| json!([]));
    }
    if matches!(
        definition.registration.request.body,
        provenance_store::operations::catalog::BodyBinding::DiscussionStart
            | provenance_store::operations::catalog::BodyBinding::DiscussionReply
    ) {
        data.entry("actor").or_insert_with(|| json!("cli"));
    }
    if definition.parameters.iter().any(|parameter| {
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
    properties.get(field).or_else(|| {
        properties
            .get(&format!("{field}s"))
            .and_then(|schema| schema.get("items"))
    })
}

fn move_scalar_to_singleton(data: &mut Map<String, Value>, source: &str, target: &str) {
    if let Some(value) = data.remove(source) {
        data.insert(target.to_owned(), json!([value]));
    }
}

fn cli_name(name: &str) -> String {
    name.to_ascii_lowercase().replace('_', "-")
}
