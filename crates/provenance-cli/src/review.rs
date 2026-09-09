mod assets;

use anyhow::Context;
use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json,
};
use provenance_core::protocol::failure::{FailureEnvelope, OperationFailure};
use provenance_transport::{HostAccess, LocalAccess, StatementHost};
use serde_json::{json, Value};
use std::{future::IntoFuture, io::Write, path::PathBuf, sync::Arc, time::Duration};

#[derive(clap::Args)]
pub struct Options {
    /// Repository with an initialized Provenance manifest.
    #[arg(long)]
    repo: PathBuf,
    /// Opaque repository target exposed to the browser.
    #[arg(long)]
    repository_id: String,
    #[arg(long)]
    scope: String,
    /// Loopback port. Zero selects an available port.
    #[arg(long, default_value_t = 0)]
    port: u16,
}

pub async fn run(options: Options) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, options.port))
        .await
        .context("cannot bind review listener")?;
    let address = listener.local_addr()?;
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let access = Arc::new(
        LocalAccess::new(
            &options.repo,
            &options.repository_id,
            &options.scope,
            &token,
            address,
        )
        .context("cannot configure review repository access")?,
    );
    let host = StatementHost::with_access(access.clone());
    let endpoint = format!("http://{address}");
    let config = json!({
        "endpoint": endpoint, "repositoryId": options.repository_id, "scope": options.scope,
        "protocolVersion": provenance_core::SDK_PROTOCOL_VERSION,
        "sdkVersion": env!("CARGO_PKG_VERSION"),
    });
    let router = host
        .router()
        .route(
            "/review-config",
            get(configuration).with_state((access.clone(), config)),
        )
        .fallback(assets::serve)
        .layer(middleware::from_fn_with_state(access, protect_origin));
    let mut signals = ShutdownSignals::new()?;
    println!(
        "{}",
        json!({
            "endpoint": endpoint, "bearer": token, "repositoryId": options.repository_id,
            "scope": options.scope, "url": format!("{endpoint}/"),
        })
    );
    std::io::stdout().flush()?;
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = stopped.await;
        })
        .into_future();
    tokio::pin!(server);
    let result = tokio::select! {
        result = &mut server => Some(result),
        () = signals.recv() => {
            let _ = stop.send(());
            None
        }
    };
    eprintln!("Review host is stopping. Wait for active operations, or send a second signal to force exit; writes may be incomplete.");
    let result = tokio::select! {
        result = async {
            // Join started operations before limiting the remaining HTTP drain.
            host.shutdown().await;
            match result {
                Some(result) => result,
                None => tokio::time::timeout(Duration::from_secs(1), &mut server)
                    .await
                    .unwrap_or(Ok(())),
            }
        } => result,
        () = signals.recv() => {
            eprintln!("Forced review host exit: writes may be incomplete. Check repository state before retrying a write.");
            // Returning would still wait for blocked tasks when Tokio drops its runtime.
            std::process::exit(1);
        }
    };
    result.context("review listener failed")
}

struct ShutdownSignals {
    #[cfg(unix)]
    terminate: tokio::signal::unix::Signal,
    #[cfg(unix)]
    interrupt: tokio::signal::unix::Signal,
}

impl ShutdownSignals {
    fn new() -> std::io::Result<Self> {
        Ok(Self {
            #[cfg(unix)]
            terminate: tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?,
            #[cfg(unix)]
            interrupt: tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?,
        })
    }

    async fn recv(&mut self) {
        #[cfg(unix)]
        tokio::select! {
            _ = self.terminate.recv() => {},
            _ = self.interrupt.recv() => {},
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
    }
}

async fn configuration(
    State((access, config)): State<(Arc<LocalAccess>, Value)>,
    request: Request,
) -> Response {
    match access.authenticate(request.headers()) {
        Ok(()) => Json(config).into_response(),
        Err(error) => refusal(error),
    }
}

async fn protect_origin(
    State(access): State<Arc<LocalAccess>>,
    request: Request,
    next: Next,
) -> Response {
    let mut response = match access.check_origin(request.headers()) {
        Ok(()) => next.run(request).await,
        Err(error) => refusal(error),
    };
    for (name, value) in [
        ("cache-control", "no-store"),
        ("referrer-policy", "no-referrer"),
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("cross-origin-resource-policy", "same-origin"),
        ("content-security-policy", "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'; worker-src 'none'"),
    ] {
        response.headers_mut().insert(name, HeaderValue::from_static(value));
    }
    response
}

fn refusal(error: OperationFailure) -> Response {
    (
        StatusCode::from_u16(error.status_code()).expect("operation failure status"),
        Json(FailureEnvelope::new(None, error)),
    )
        .into_response()
}
