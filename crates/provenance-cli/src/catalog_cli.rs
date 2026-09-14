//! Generated-dialect CLI dispatch over the registered resource catalog.
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method};
use provenance_store::operations::catalog::{self, Definition};
use serde_json::{json, Map, Value};
use std::{
    collections::BTreeMap,
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
    let (method, path, route_words) = address(&collection, words)?;
    let definition = catalog::definitions()
        .into_iter()
        .find(|definition| {
            definition
                .method
                .as_str()
                .eq_ignore_ascii_case(method.as_str())
                && path_matches(definition.path, &path)
        })
        .ok_or_else(|| anyhow::anyhow!("the catalog does not declare {method} {path}"))?;
    let (data, query, headers) = input(&definition, route_words)?;
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
                    "--repo" => repo = value.clone(),
                    "--scope" => scope = value.clone(),
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

fn address(
    collection: &str,
    mut words: Vec<String>,
) -> anyhow::Result<(Method, String, Vec<String>)> {
    let first = words
        .first()
        .map(String::as_str)
        .ok_or_else(|| anyhow::anyhow!("{collection} requires list, create, an id, or a query"))?;
    let (method, path, consumed) = match first {
        "list" => (Method::GET, format!("/{collection}"), 1),
        "create" => (Method::POST, format!("/{collection}"), 1),
        "search" | "stale" | "resolve-symbol" => (Method::GET, format!("/{collection}"), 0),
        "begin-verification" if collection == "verification-runs" => (
            Method::POST,
            "/verification-runs/begin-verification".into(),
            1,
        ),
        "get" | "update" => {
            let id = words
                .get(1)
                .ok_or_else(|| anyhow::anyhow!("{first} requires a resource id"))?;
            (
                if first == "get" {
                    Method::GET
                } else {
                    Method::PATCH
                },
                format!("/{collection}/{id}"),
                2,
            )
        }
        id => {
            let action = words.get(1).map(String::as_str).unwrap_or("get");
            member_address(collection, id, action, &words)?
        }
    };
    if consumed > 0 {
        words.drain(0..consumed);
    }
    Ok((method, path, words))
}

fn member_address(
    collection: &str,
    id: &str,
    action: &str,
    words: &[String],
) -> anyhow::Result<(Method, String, usize)> {
    let base = format!("/{collection}/{id}");
    match action {
        "get" => Ok((Method::GET, base, usize::from(words.get(1).is_some()) + 1)),
        "update" => Ok((Method::PATCH, base, 2)),
        "trace" | "neighbors" | "impact" => Ok((Method::GET, base, 1)),
        "claim" | "release" | "close" | "answer" | "submit" => {
            Ok((Method::POST, format!("{base}/{action}"), 2))
        }
        "complete-verification" if collection == "verification-runs" => {
            Ok((Method::POST, format!("{base}/complete-verification"), 2))
        }
        "document" | "evidence" => Ok((Method::GET, format!("{base}/{action}"), 2)),
        "history" => history_address(&base, words),
        "submissions" if collection == "requirements" => submission_address(&base, words),
        "assertions" | "dispositions" if collection == "proposals" => {
            fact_address(&base, action, words)
        }
        "discussions" => discussion_address(&base, words),
        "discussion-containers" => legacy_message_address(&base, words),
        _ => anyhow::bail!("unknown {collection} member action: {action}"),
    }
}

fn history_address(base: &str, words: &[String]) -> anyhow::Result<(Method, String, usize)> {
    let Some(entry) = words.get(2) else {
        return Ok((Method::GET, format!("{base}/history"), 2));
    };
    let mut path = format!("{base}/history/{entry}");
    let mut consumed = 3;
    if words.get(3).is_some_and(|word| word == "evidence") {
        let side = words
            .get(4)
            .ok_or_else(|| anyhow::anyhow!("history evidence requires before or after"))?;
        path.push_str(&format!("/evidence/{side}"));
        consumed = 5;
    }
    Ok((Method::GET, path, consumed))
}

fn submission_address(base: &str, words: &[String]) -> anyhow::Result<(Method, String, usize)> {
    let proposal = words
        .get(2)
        .ok_or_else(|| anyhow::anyhow!("submissions requires a Proposal id"))?;
    let action = words
        .get(3)
        .ok_or_else(|| anyhow::anyhow!("submission requires decide or withdraw"))?;
    anyhow::ensure!(
        matches!(action.as_str(), "decide" | "withdraw"),
        "submission action must be decide or withdraw"
    );
    Ok((
        Method::POST,
        format!("{base}/submissions/{proposal}/{action}"),
        4,
    ))
}

fn fact_address(
    base: &str,
    kind: &str,
    words: &[String],
) -> anyhow::Result<(Method, String, usize)> {
    match words.get(2).map(String::as_str) {
        None | Some("list") => Ok((Method::GET, format!("{base}/{kind}"), words.len().min(3))),
        Some("create") => Ok((Method::POST, format!("{base}/{kind}"), 3)),
        Some(id) => Ok((Method::GET, format!("{base}/{kind}/{id}"), 3)),
    }
}

fn discussion_address(base: &str, words: &[String]) -> anyhow::Result<(Method, String, usize)> {
    match words.get(2).map(String::as_str) {
        None | Some("list") => Ok((
            Method::GET,
            format!("{base}/discussions"),
            words.len().min(3),
        )),
        Some("create") => Ok((Method::POST, format!("{base}/discussions"), 3)),
        Some(discussion) => {
            let path = format!("{base}/discussions/{discussion}");
            match words.get(3).map(String::as_str) {
                None | Some("get") => Ok((Method::GET, path, words.len().min(4))),
                Some("update") => Ok((Method::PATCH, path, 4)),
                Some("messages") => match words.get(4).map(String::as_str) {
                    None | Some("list") => {
                        Ok((Method::GET, format!("{path}/messages"), words.len().min(5)))
                    }
                    Some("create") => Ok((Method::POST, format!("{path}/messages"), 5)),
                    Some(message) => Ok((Method::GET, format!("{path}/messages/{message}"), 5)),
                },
                Some(other) => anyhow::bail!("unknown Discussion action: {other}"),
            }
        }
    }
}

fn legacy_message_address(base: &str, words: &[String]) -> anyhow::Result<(Method, String, usize)> {
    let container = words
        .get(2)
        .ok_or_else(|| anyhow::anyhow!("discussion-containers requires a container id"))?;
    anyhow::ensure!(
        words.get(3).is_some_and(|word| word == "legacy-messages"),
        "expected legacy-messages after the container id"
    );
    let path = format!("{base}/discussion-containers/{container}/legacy-messages");
    Ok(match words.get(4) {
        Some(message) => (Method::GET, format!("{path}/{message}"), 5),
        None => (Method::GET, path, 4),
    })
}

fn path_matches(pattern: &str, actual: &str) -> bool {
    let expected = pattern.split('/').filter(|part| !part.is_empty());
    let actual = actual.split('/').filter(|part| !part.is_empty());
    expected.clone().count() == actual.clone().count()
        && expected.zip(actual).all(|(expected, actual)| {
            expected.starts_with('{') && expected.ends_with('}') || expected == actual
        })
}

fn input(
    definition: &Definition,
    words: Vec<String>,
) -> anyhow::Result<(Value, BTreeMap<String, String>, HeaderMap)> {
    let mut data = Map::new();
    let mut query = BTreeMap::new();
    let mut headers = HeaderMap::new();
    let mut stdin = false;
    let mut index = 0;
    let query_action = words
        .first()
        .filter(|word| {
            matches!(
                word.as_str(),
                "search" | "stale" | "resolve-symbol" | "trace" | "neighbors" | "impact"
            )
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
        } else {
            anyhow::ensure!(
                definition.request_schema.is_some(),
                "{definition_name} has no request body",
                definition_name = definition.name
            );
            data.insert(flag.replace('-', "_"), scalar(value)?);
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
        rename_scalar(&mut data, "method", "resolution_method");
        data.entry("status").or_insert_with(|| json!("open"));
        data.entry("links").or_insert_with(|| json!([]));
    }
    if definition.name.ends_with("create-discussion")
        || definition.name.ends_with("create-discussion-message")
    {
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

fn move_scalar_to_singleton(data: &mut Map<String, Value>, source: &str, target: &str) {
    if let Some(value) = data.remove(source) {
        data.insert(target.to_owned(), json!([value]));
    }
}

fn rename_scalar(data: &mut Map<String, Value>, source: &str, target: &str) {
    if let Some(value) = data.remove(source) {
        data.insert(target.to_owned(), value);
    }
}

fn scalar(value: &str) -> anyhow::Result<Value> {
    Ok(match value {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        value if value.parse::<i64>().is_ok() => Value::from(value.parse::<i64>()?),
        value if value.parse::<f64>().is_ok() => Value::from(value.parse::<f64>()?),
        value if value.starts_with('[') || value.starts_with('{') => {
            anyhow::bail!("arrays and objects must come from --stdin")
        }
        value => Value::String(value.to_owned()),
    })
}

fn cli_name(name: &str) -> String {
    name.to_ascii_lowercase().replace('_', "-")
}

#[cfg(test)]
#[path = "catalog_cli/tests.rs"]
mod tests;
