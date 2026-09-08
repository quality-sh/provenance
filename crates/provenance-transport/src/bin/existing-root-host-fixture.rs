//! Explicit writable test host for one harness-owned repository.
use provenance_transport::{
    fixture::{FixtureAccess, Target},
    StatementHost,
};
use serde_json::json;
use std::{io::Write, path::PathBuf};
use tokio::io::AsyncReadExt;

fn setting(name: &str) -> Result<String, std::io::Error> {
    std::env::var(name)
        .map_err(|_| std::io::Error::other(format!("missing fixture setting {name}")))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(setting("PROVENANCE_FIXTURE_ROOT")?);
    let repository = setting("PROVENANCE_FIXTURE_REPOSITORY_ID")?;
    let scope = setting("PROVENANCE_FIXTURE_SCOPE")?;
    let token = setting("PROVENANCE_FIXTURE_TOKEN")?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let authority = listener.local_addr()?.to_string();
    let access = FixtureAccess::new(
        vec![Target {
            id: repository.clone(),
            root,
        }],
        vec![(repository.clone(), scope.clone())],
        &token,
        &authority,
    )?
    .allow_writes();
    let host = StatementHost::with_fixture_access(access);
    println!(
        "{}",
        json!({"url": format!("http://{authority}"), "repository": repository, "scope": scope})
    );
    std::io::stdout().flush()?;
    let closing = host.clone();
    axum::serve(listener, host.router())
        .with_graceful_shutdown(async move {
            let mut bytes = [0; 256];
            while matches!(tokio::io::stdin().read(&mut bytes).await, Ok(count) if count > 0) {}
            closing.shutdown().await;
        })
        .await?;
    host.shutdown().await;
    Ok(())
}
