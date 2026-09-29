use super::OutputFormat;
use crate::repo_context::RepoContext;
use provenance_core::protocol::SearchQuery;

/// Run one root search through the canonical typed operation.
pub async fn dispatch_search(
    repo: &str,
    scope: &str,
    format: Option<OutputFormat>,
    query: SearchQuery,
) -> anyhow::Result<()> {
    let host = RepoContext::new(repo, scope).local_host()?;
    let service = host.porcelain().search();
    let response = service.search(query).await?;
    let rendered = if format == Some(OutputFormat::Json) {
        serde_json::to_string_pretty(&response)?
    } else {
        provenance_porcelain::search::render_readable(&response)
    };
    println!("{rendered}");
    Ok(())
}
