use super::{local_host, OutputFormat};
use provenance_core::protocol::SearchQuery;

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
        provenance_porcelain::search::render_readable(&response)
    };
    println!("{rendered}");
    Ok(())
}
