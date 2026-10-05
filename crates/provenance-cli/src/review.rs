mod assets;
pub mod launch;

use anyhow::Context;
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json,
};
use provenance_core::protocol::failure::{FailureEnvelope, OperationFailure};
use provenance_transport::{
    local_host::{
        LocalHostIdentity, LocalHostRegistration, IDENTITY_ROUTE, LAUNCH_CODE_ROUTE,
        LAUNCH_SESSION_ROUTE,
    },
    HostAccess, LocalAccess, StatementHost,
};
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
    /// Do not open the review page in a browser.
    #[arg(long)]
    no_open: bool,
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
    if access.disposition_actor_ids()?.is_empty() {
        eprintln!("{}", crate::reviewer::review_page_warning(&options.repo));
    }
    let host = StatementHost::with_access(access.clone()).with_check_port(Arc::new(
        crate::handlers::check::RepositoryCheckPort::new(
            camino::Utf8PathBuf::from_path_buf(options.repo.clone())
                .map_err(|_| anyhow::anyhow!("repository path is not UTF-8"))?,
            false,
            None,
        ),
    ));
    let endpoint = format!("http://{address}");
    let runtime = LocalHostRegistration::publish(
        &options.repo,
        &options.scope,
        &endpoint,
        &options.repository_id,
    )?;
    let identity = runtime.identity();
    // The codes own the launch key, so the key file is removed when the host stops.
    let codes = launch::LaunchCodes::new(
        &token,
        launch::LaunchKey::publish(&identity.instance_nonce)?,
    );
    let config = json!({
        "endpoint": endpoint, "repositoryId": options.repository_id, "scope": options.scope,
        "compatibility": provenance_core::protocol::host::COMPATIBILITY,
        "sdkVersion": env!("CARGO_PKG_VERSION"),
    });
    let review_configuration = ReviewConfiguration {
        access: access.clone(),
        value: config,
    };
    let router = host
        .router()
        .route(
            IDENTITY_ROUTE,
            get(local_host_identity).with_state(identity.clone()),
        )
        .route(
            "/review-config",
            get(configuration).with_state(review_configuration),
        )
        .route(
            LAUNCH_CODE_ROUTE,
            post(launch::issue_code).with_state(codes.clone()),
        )
        .route(
            LAUNCH_SESSION_ROUTE,
            post(launch::open_session).with_state(codes.clone()),
        )
        .fallback(assets::serve)
        .layer(middleware::from_fn_with_state(access, protect_origin));
    let signals = ShutdownSignals::new()?;
    println!(
        "{}",
        json!({
            "endpoint": endpoint, "bearer": token, "repositoryId": options.repository_id,
            "scope": options.scope, "instanceNonce": identity.instance_nonce,
            "url": format!("{endpoint}/"),
        })
    );
    std::io::stdout().flush()?;
    open_review_page(&endpoint, &codes, options.no_open)?;
    serve(listener, router, host, signals).await
}

/// Opens the review page signed in, or tells the person which link to open.
fn open_review_page(
    endpoint: &str,
    codes: &launch::LaunchCodes,
    no_open: bool,
) -> anyhow::Result<()> {
    if no_open {
        return Ok(());
    }
    let mut link = url::Url::parse(&format!("{endpoint}/"))?;
    link.set_fragment(Some(&format!(
        "launch={}",
        codes.issue(std::time::Instant::now())
    )));
    // A browser program can stay open, so the host does not wait for it before it serves.
    tokio::task::spawn_blocking(move || {
        if let crate::browser::Opening::Printed(reason) =
            crate::browser::open_or_print(&link, false)
        {
            eprintln!(
                "The review page did not open because {}. Open this link in your browser: {link}",
                reason.reason()
            );
        }
    });
    Ok(())
}

async fn serve(
    listener: tokio::net::TcpListener,
    router: axum::Router,
    host: StatementHost,
    mut signals: ShutdownSignals,
) -> anyhow::Result<()> {
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

async fn local_host_identity(State(identity): State<LocalHostIdentity>) -> Json<LocalHostIdentity> {
    Json(identity)
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

#[derive(Clone)]
struct ReviewConfiguration {
    access: Arc<LocalAccess>,
    value: Value,
}

async fn configuration(
    State(mut config): State<ReviewConfiguration>,
    request: Request,
) -> Response {
    if let Err(error) = config.access.authenticate(request.headers()) {
        return refusal(error);
    }
    let actor_ids = match config.access.disposition_actor_ids() {
        Ok(actor_ids) => actor_ids,
        Err(error) => return refusal(error),
    };
    config.value["dispositionActorIds"] = json!(actor_ids);
    Json(config.value).into_response()
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
    if matches!(
        response.status(),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) {
        response
            .headers_mut()
            .insert(header::CONNECTION, HeaderValue::from_static("close"));
    }
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
