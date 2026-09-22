use super::{local_host, OutputFormat};
use provenance_core::protocol::{QueryResponse, SearchQuery, SearchResult, QUERY_DEFAULT_LIMIT};
use provenance_core::{NodeType, SDK_PROTOCOL_VERSION};
use std::fmt::{Display, Formatter};

/// One parsed root-search action.
pub enum SearchCommand {
    Help,
    Run(SearchQuery),
}

/// Invalid root-search grammar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchBindingError;

impl Display for SearchBindingError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("unsupported search options")
    }
}

impl std::error::Error for SearchBindingError {}

/// Translate root-search words into the canonical typed query.
pub fn parse_search(words: &[&str]) -> Result<SearchCommand, SearchBindingError> {
    if words == ["search", "--help"] {
        return Ok(SearchCommand::Help);
    }
    if words.first().copied() != Some("search") {
        return Err(SearchBindingError);
    }
    let mut text = None;
    let mut cursor = None;
    let mut node_types = Vec::new();
    let mut limit = None;
    let mut index = 1;
    while index < words.len() {
        let value = words.get(index + 1).copied().ok_or(SearchBindingError)?;
        match words[index] {
            "--text" if text.is_none() => text = Some(value.to_owned()),
            "--cursor" if cursor.is_none() => cursor = Some(value.to_owned()),
            "--kind" => node_types.push(NodeType::parse(value).map_err(|_| SearchBindingError)?),
            "--limit" if limit.is_none() => {
                limit = Some(value.parse().map_err(|_| SearchBindingError)?);
            }
            _ => return Err(SearchBindingError),
        }
        index += 2;
    }
    Ok(SearchCommand::Run(SearchQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        cursor,
        text,
        node_types,
        limit: limit.unwrap_or(QUERY_DEFAULT_LIMIT),
    }))
}

/// Run one root search through the canonical typed operation.
pub async fn dispatch_search(
    repo: &str,
    scope: &str,
    format: Option<OutputFormat>,
    query: SearchQuery,
) -> anyhow::Result<()> {
    let host = local_host(repo, scope)?;
    let service = provenance_porcelain::Porcelain::new(
        provenance_transport::porcelain::HostSearchPort::new(host),
    );
    let response = service.search(query).await?;
    let rendered = if format == Some(OutputFormat::Json) {
        serde_json::to_string_pretty(&response)?
    } else {
        render_readable(&response)
    };
    println!("{rendered}");
    Ok(())
}

fn render_readable(response: &QueryResponse<SearchResult>) -> String {
    let result = &response.result;
    let mut lines = vec![format!("search: {} returned", result.nodes.len())];
    for node in &result.nodes {
        lines.push(format!(
            "- {} {}\n  {}",
            node.node_type().as_str(),
            node.id().as_str(),
            node.searchable_text().get(1).copied().unwrap_or("")
        ));
    }
    lines.push(format!(
        "bounds: limit={} has_more={} continuation={}",
        result.limit,
        result.has_more,
        result.next_cursor.as_deref().unwrap_or("none")
    ));
    if let Some(error) = &response.freshness_error {
        lines.push(format!("warning: freshness: {error}"));
    }
    lines.join("\n")
}

/// Print the root-search grammar without opening a repository.
pub fn print_search_help() {
    println!("Search records in one scope:");
    println!("  provenance search [--text <text>] [--kind <record-type>]...");
    println!("                    [--limit <count>] [--cursor <token>]");
    println!("                    [--repo <path>] [--scope <id>] [--format json]");
}
