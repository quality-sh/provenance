//! CLI-owned bindings for shared Porcelain capabilities.

use provenance_macros::rule;
use provenance_porcelain::check::{Category, CheckInput, CheckOutcome};
use provenance_porcelain::get::{GetInput, GetOutcome, View};
use std::{
    fmt::{Display, Formatter},
    net::{Ipv4Addr, SocketAddr},
};

/// An explicit CLI output format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    /// Emit JSON result data.
    Json,
}

/// An invalid CLI Porcelain binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BindingError;

impl Display for BindingError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("unsupported read options")
    }
}

impl std::error::Error for BindingError {}

/// Translate the live CLI selector fields into one semantic check request.
pub fn check_input_from_selectors(graph: bool, statements: bool, bindings: bool) -> CheckInput {
    let mut categories = Vec::new();
    if graph {
        categories.push(Category::Graph);
    }
    if statements {
        categories.push(Category::Statements);
    }
    if bindings {
        categories.push(Category::Bindings);
    }
    CheckInput::new(categories)
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
            "--kind" => input.returned_kinds.push(
                provenance_core::NodeType::parse(value)
                    .map_err(|_| BindingError)?,
            ),
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

/// Select the default get action without reading repository state.
#[rule("rule_porcelain_get_is_default_action")]
pub fn bare_target_selects_get(arguments: &[String], is_builtin: bool) -> bool {
    let words = raw_words(arguments);
    !is_builtin && !words.is_empty() && words.get(1).is_none_or(|word| word.starts_with("--"))
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

/// Renders the selected record as readable text or structured JSON.
fn render_get(outcome: &GetOutcome, format: Option<OutputFormat>) -> serde_json::Result<String> {
    if format == Some(OutputFormat::Json) {
        serde_json::to_string_pretty(outcome)
    } else {
        render_get_readable(outcome)
    }
}

/// Render a get outcome and report any stale response metadata.
pub fn render_get_readable(outcome: &GetOutcome) -> serde_json::Result<String> {
    let sections = vec![
        format!(
            "{} {}",
            outcome.record.node_type().as_str(),
            outcome.record.id()
        ),
        format!("view: {:?}", outcome.view()).to_ascii_lowercase(),
        format!(
            "record:\n{}",
            serde_json::to_string_pretty(&provenance_porcelain::get::RecordData(&outcome.record))?
        ),
    ];
    let mut sections = sections;
    if !outcome.related().is_empty() {
        sections.push(format!(
            "related:\n{}",
            outcome
                .related()
                .iter()
                .map(|record| format!(
                    "- {} {}: {}",
                    record.node.node_type().as_str(),
                    record.node.id(),
                    serde_json::to_string(&provenance_porcelain::get::RecordData(&record.node))
                        .expect("record values are valid JSON")
                ))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    if let Some(detail) = outcome.impact() {
        sections.push(format!(
            "detail:\n{}",
            serde_json::to_string_pretty(detail)?
        ));
    }
    if let Some(bounds) = outcome.bounds() {
        sections.push(format!(
            "bounds: limit={} max_depth={} has_more={} truncated={} continuation={}",
            bounds.limit,
            bounds
                .max_depth
                .map_or_else(|| "none".to_owned(), |depth| depth.to_string()),
            bounds.has_more,
            bounds.truncated,
            bounds.continuation.as_deref().unwrap_or("none")
        ));
    }
    Ok(finish_readable(outcome, sections))
}

fn finish_readable(outcome: &GetOutcome, mut sections: Vec<String>) -> String {
    for (label, metadata) in [
        ("record", outcome.record_metadata.as_ref()),
        ("view", outcome.view_metadata()),
    ] {
        if let Some(error) = metadata.and_then(|value| value.freshness_error.as_deref()) {
            sections.push(format!("warning: {label} freshness: {error}"));
        }
    }
    sections.join("\n\n")
}
