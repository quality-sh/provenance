use super::OutputFormat;
use crate::repo_context::RepoContext;
use provenance_porcelain::search::SearchInput;

/// Run one root search through the canonical typed operation.
pub async fn dispatch_search(
    repo: &str,
    scope: &str,
    format: Option<OutputFormat>,
    query: SearchInput,
) -> anyhow::Result<()> {
    let host = RepoContext::new(repo, scope).local_host()?;
    let service = host.porcelain().search();
    let outcome = service.search(query).await?;
    let rendered = if format == Some(OutputFormat::Json) {
        serde_json::to_string_pretty(&outcome)?
    } else {
        provenance_porcelain::search::render_readable(&outcome)
    };
    println!("{rendered}");
    Ok(())
}
