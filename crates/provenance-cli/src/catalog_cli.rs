//! Generated-dialect CLI dispatch over the registered resource catalog.
use axum::http::Method;
use provenance_store::operations::catalog;
use std::net::{Ipv4Addr, SocketAddr};

mod address;
mod input;

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
    if words.as_slice() == ["--help"] {
        print_help(&collection);
        return Ok(true);
    }
    let resolved = address::resolve(&collection, &words)?;
    if resolved.flags.as_slice() == ["--help"] {
        print_help(&collection);
        return Ok(true);
    }
    let definition = resolved.definition;
    let method = match definition.method {
        catalog::HttpMethod::Get => Method::GET,
        catalog::HttpMethod::Post => Method::POST,
        catalog::HttpMethod::Patch => Method::PATCH,
    };
    let (data, query, headers) = input::parse(definition, &resolved.flags, resolved.query)?;
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
