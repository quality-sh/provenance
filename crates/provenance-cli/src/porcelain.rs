//! CLI-owned bindings for shared Porcelain capabilities.

use provenance_porcelain::check::{Category, CheckInput, CheckOutcome};
use provenance_porcelain::get::{GetInput, GetOutcome, View};
use provenance_porcelain::{Action, Outcome, RecordRequest};
use serde::Serialize;
use std::{
    fmt::{Display, Formatter},
    net::{Ipv4Addr, SocketAddr},
};

const ACTION_NAMES: &[&str] = &["get", "check"];

/// Return the Porcelain action names exposed by the CLI binding.
pub const fn action_names() -> &'static [&'static str] {
    ACTION_NAMES
}

/// An explicit CLI output format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    /// Emit JSON result data.
    Json,
}

/// Render a shared outcome in the CLI-owned output shape.
pub fn render<T: Serialize>(
    outcome: &Outcome<T>,
    format: Option<OutputFormat>,
) -> serde_json::Result<String> {
    match format {
        None => Ok(outcome.summary.clone()),
        Some(OutputFormat::Json) => serde_json::to_string_pretty(&outcome.data),
    }
}

/// An invalid CLI Porcelain binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BindingError;

impl Display for BindingError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("use <target> get")
    }
}

impl std::error::Error for BindingError {}

/// Translate target-first CLI words into one semantic record request.
pub fn parse_record(words: &[&str]) -> Result<RecordRequest, BindingError> {
    match words {
        [target, "get"] if *target != "get" => Ok(RecordRequest::new(*target, Action::Get)),
        [target] if !target.is_empty() => Ok(RecordRequest::new(*target, Action::Get)),
        _ => Err(BindingError),
    }
}

/// Translate CLI-owned selector flags into one semantic check request.
pub fn parse_check(words: &[&str]) -> Result<CheckInput, BindingError> {
    let mut categories = Vec::new();
    for word in words {
        categories.push(match *word {
            "--graph" => Category::Graph,
            "--statements" => Category::Statements,
            "--bindings" => Category::Bindings,
            _ => return Err(BindingError),
        });
    }
    Ok(CheckInput::new(categories))
}

/// Render a semantic check result for a terminal reader.
pub fn render_check(outcome: &CheckOutcome) -> String {
    outcome
        .categories
        .iter()
        .flat_map(|report| {
            let heading =
                format!("{:?}: {:?}", report.category, report.status).to_ascii_lowercase();
            std::iter::once(heading)
                .chain(
                    report
                        .findings
                        .iter()
                        .map(|finding| format!("  - {}", finding.message)),
                )
                .chain(
                    report
                        .unavailable_reason
                        .iter()
                        .map(|reason| format!("  - {reason}")),
                )
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Translate CLI-owned get words into one semantic request.
pub fn parse_get(words: &[&str]) -> Result<GetInput, BindingError> {
    let (target, mut index) = match words {
        [target, rest @ ..] if !target.is_empty() && rest.first().copied() != Some("get") => {
            (*target, 1)
        }
        [target, "get", ..] if !target.is_empty() => (*target, 2),
        _ => return Err(BindingError),
    };
    let mut input = GetInput::new(target, View::Record);
    while index < words.len() {
        let value = words.get(index + 1).copied().ok_or(BindingError)?;
        match words[index] {
            "--view" => {
                input.view = match value {
                    "record" => View::Record,
                    "children" => View::Children,
                    "grounding" => View::Grounding,
                    "impact" => View::Impact,
                    _ => return Err(BindingError),
                };
            }
            "--depth" => input.max_depth = Some(value.parse().map_err(|_| BindingError)?),
            "--kind" => input.returned_kinds.push(value.to_owned()),
            "--limit" => input.limit = Some(value.parse().map_err(|_| BindingError)?),
            _ => return Err(BindingError),
        }
        index += 2;
    }
    Ok(input)
}

/// Run a target-first Porcelain get command when the arguments select one.
pub async fn try_dispatch(arguments: &[String]) -> anyhow::Result<bool> {
    let Some(invocation) = split_get_arguments(arguments)? else {
        return Ok(false);
    };
    let input = parse_get(
        &invocation
            .words
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    )
    .map_err(|error| anyhow::anyhow!(error))?;
    let root = std::fs::canonicalize(invocation.repo)?;
    let access = provenance_transport::LocalAccess::new(
        &root,
        "native",
        &invocation.scope,
        &"0".repeat(64),
        SocketAddr::from((Ipv4Addr::LOCALHOST, 1)),
    )
    .map_err(|failure| anyhow::anyhow!(failure))?;
    let host = provenance_transport::StatementHost::with_access(std::sync::Arc::new(access));
    let service = provenance_porcelain::Porcelain::new(
        provenance_transport::porcelain::HostGetPort::new(host),
    );
    let outcome = service.get(input).await?;
    println!("{}", render_get(&outcome, invocation.format)?);
    Ok(true)
}

/// Report whether the arguments explicitly select the target-first get grammar.
pub fn explicitly_selects_get(arguments: &[String]) -> anyhow::Result<bool> {
    if raw_words(arguments)
        .get(1)
        .is_none_or(|word| word.as_str() != "get")
    {
        return Ok(false);
    }
    let Some(invocation) = split_get_arguments(arguments)? else {
        return Ok(false);
    };
    if invocation.words.get(1).map(String::as_str) != Some("get") {
        return Ok(false);
    }
    Ok(parse_get(
        &invocation
            .words
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    )
    .is_ok())
}

fn raw_words(arguments: &[String]) -> Vec<&String> {
    let mut words = Vec::new();
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--quiet" => index += 1,
            "--repo" | "--scope" | "--format" => index += 2,
            _ => {
                words.push(&arguments[index]);
                index += 1;
            }
        }
    }
    words
}

struct GetInvocation {
    repo: String,
    scope: String,
    format: Option<OutputFormat>,
    words: Vec<String>,
}

fn split_get_arguments(arguments: &[String]) -> anyhow::Result<Option<GetInvocation>> {
    let mut repo = ".".to_owned();
    let mut scope = "default".to_owned();
    let mut format = None;
    let mut words = Vec::new();
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--quiet" => index += 1,
            "--repo" | "--scope" | "--format" => {
                let value = arguments
                    .get(index + 1)
                    .ok_or_else(|| anyhow::anyhow!("{} requires a value", arguments[index]))?;
                match arguments[index].as_str() {
                    "--repo" => repo.clone_from(value),
                    "--scope" => scope.clone_from(value),
                    _ if value == "json" => format = Some(OutputFormat::Json),
                    _ => anyhow::bail!("Porcelain get supports --format json"),
                }
                index += 2;
            }
            value => {
                words.push(value.to_owned());
                index += 1;
            }
        }
    }
    if words.is_empty() {
        return Ok(None);
    }
    Ok(Some(GetInvocation {
        repo,
        scope,
        format,
        words,
    }))
}

fn render_get(outcome: &GetOutcome, format: Option<OutputFormat>) -> serde_json::Result<String> {
    if format == Some(OutputFormat::Json) {
        serde_json::to_string_pretty(outcome)
    } else {
        Ok(format!("{} {}", outcome.record.kind, outcome.record.id))
    }
}
