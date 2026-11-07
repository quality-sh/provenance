//! The `docs serve` command: the docs site of a repository as HTML, after the link check
//! that `check --docs` runs.

use anyhow::Context;
use camino::Utf8PathBuf;
use provenance_transport::docs::DocsSite;

mod web;

pub async fn serve(repo: Utf8PathBuf, host: String, port: u16) -> anyhow::Result<()> {
    let site = DocsSite::load(repo.as_std_path())?;
    site.ensure_links_valid()?;
    let app = web::router(site);
    let listener = tokio::net::TcpListener::bind((host.as_str(), port))
        .await
        .with_context(|| format!("failed to bind docs server to {host}:{port}"))?;
    eprintln!(
        "serving Provenance docs at http://{}",
        listener.local_addr()?
    );
    axum::serve(listener, app).await?;
    Ok(())
}
