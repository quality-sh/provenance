//! Explicitly enabled local fixture for generated-client tests.
use provenance_transport::StatementHost;
use std::io::Write;
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = StatementHost::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    println!("http://{}", listener.local_addr()?);
    std::io::stdout().flush()?;
    let closing_host = host.clone();
    axum::serve(listener, host.router())
        .with_graceful_shutdown(async move {
            let mut buffer = [0; 256];
            while matches!(tokio::io::stdin().read(&mut buffer).await, Ok(n) if n > 0) {}
            closing_host.shutdown().await;
        })
        .await?;
    host.shutdown().await;
    Ok(())
}
