//! CLI-owned bindings for shared Porcelain capabilities.

use provenance_porcelain::check::{Category, CheckInput};
use provenance_porcelain::get::{GetInput, GetOutcome};
use std::net::{Ipv4Addr, SocketAddr};

mod search;
pub use provenance_porcelain::check::render_readable as render_check;
pub use provenance_porcelain::get::render_readable as render_get_readable;
pub use search::dispatch_search;

/// An explicit CLI output format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    /// Emit JSON result data.
    Json,
}
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

/// Run one fully parsed target-first Porcelain get command.
pub async fn dispatch_get(
    repo: &str,
    scope: &str,
    format: Option<OutputFormat>,
    input: GetInput,
) -> anyhow::Result<()> {
    let host = local_host(repo, scope)?;
    let service = provenance_porcelain::Porcelain::new(
        provenance_transport::porcelain::HostGetPort::new(host),
    );
    let outcome = service.get(input).await?;
    println!("{}", render_get(&outcome, format)?);
    Ok(())
}

pub fn local_host(repo: &str, scope: &str) -> anyhow::Result<provenance_transport::StatementHost> {
    provenance_store::layout::require_initialized_graph(
        &provenance_store::layout::ProvenanceLayout::new(repo),
    )?;
    let root = std::fs::canonicalize(repo)?;
    let access = provenance_transport::LocalAccess::new(
        &root,
        "native",
        scope,
        &"0".repeat(64),
        SocketAddr::from((Ipv4Addr::LOCALHOST, 1)),
    )
    .map_err(|failure| anyhow::anyhow!(failure))?;
    Ok(provenance_transport::StatementHost::with_access(
        std::sync::Arc::new(access),
    ))
}

/// Renders the selected record as readable text or structured JSON.
fn render_get(outcome: &GetOutcome, format: Option<OutputFormat>) -> serde_json::Result<String> {
    if format == Some(OutputFormat::Json) {
        serde_json::to_string_pretty(outcome)
    } else {
        render_get_readable(outcome)
    }
}
