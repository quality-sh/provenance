mod assets;
mod detach;
mod events;
pub mod follow;
pub mod launch;

use anyhow::Context;
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json,
};
use provenance_core::protocol::failure::{FailureEnvelope, OperationFailure};
use provenance_store::operations::LocalRepository;
use provenance_transport::{
    local_host::{
        LocalHostIdentity, LocalHostRegistration, IDENTITY_ROUTE, LAUNCH_CODE_ROUTE,
        LAUNCH_SESSION_ROUTE,
    },
    HttpCredential, LocalAccess, StatementHost,
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
    /// Reuse the running host for this repository and scope, or start one in the background
    /// that keeps running after this command returns. Prints where the review page is served.
    #[arg(long)]
    detach: bool,
}

pub async fn run(options: Options) -> anyhow::Result<()> {
    if options.detach {
        return detach::run(&options);
    }
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, options.port))
        .await
        .context("cannot bind review listener")?;
    let address = listener.local_addr()?;
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let (canonical_repo, repository, access) = repository_access(&options, &token, address)?;
    if repository.disposition_actor_ids()?.is_empty() {
        eprintln!("{}", crate::reviewer::review_page_warning(&options.repo));
    }
    let host = StatementHost::with_access(repository.clone())
        .with_credential(access.clone())
        .with_check_port(Arc::new(crate::handlers::check::RepositoryCheckPort::new(
            camino::Utf8PathBuf::from_path_buf(canonical_repo.clone())
                .map_err(|_| anyhow::anyhow!("repository path is not UTF-8"))?,
            false,
            None,
        )));
    let endpoint = format!("http://{address}");
    // Discovery treats a registered host that does not answer as stale, so the record is
    // announced only after the listener serves.
    let registration = LocalHostRegistration::prepare(
        &canonical_repo,
        &options.scope,
        &endpoint,
        &options.repository_id,
    )?;
    let identity = registration.identity();
    // The codes own the launch key, so the key file is removed when the host stops.
    let codes = launch::LaunchCodes::new(
        &token,
        launch::LaunchKey::publish(&identity.instance_nonce)?,
    );
    let events = events::ReviewEvents::start(
        provenance_store::layout::ProvenanceLayout::new(
            camino::Utf8PathBuf::from_path_buf(canonical_repo.clone())
                .map_err(|_| anyhow::anyhow!("repository path is not UTF-8"))?,
        ),
        options.scope.clone(),
        identity.instance_nonce.clone(),
        access.clone(),
        codes.clone(),
    )
    .await;
    let config = json!({
        "endpoint": endpoint, "repositoryId": options.repository_id, "scope": options.scope,
        "compatibility": provenance_core::protocol::host::COMPATIBILITY,
        "sdkVersion": env!("CARGO_PKG_VERSION"),
    });
    let review_configuration = ReviewConfiguration {
        credential: access.clone(),
        repository,
        path: camino::Utf8PathBuf::from_path_buf(canonical_repo.clone())
            .map_err(|_| anyhow::anyhow!("repository path is not UTF-8"))?,
        value: config,
    };
    let page_events = events.clone();
    let host = host.with_page_writes(
        provenance_store::layout::ProvenanceLayout::new(
            camino::Utf8PathBuf::from_path_buf(canonical_repo)
                .map_err(|_| anyhow::anyhow!("repository path is not UTF-8"))?,
        ),
        Arc::new(move |changes| page_events.page_changed(changes)),
    );
    let router = review_router(
        &host,
        &identity,
        review_configuration,
        &codes,
        &events,
        access,
        address,
    );
    let signals = ShutdownSignals::new()?;
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .into_future(),
    );
    tokio::task::yield_now().await;
    registration.announce()?;
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
    serve(server, stop, host, events, signals).await
}

/// The host routes plus the review page's identity, configuration, launch, event, and
/// asset routes.
fn review_router(
    host: &StatementHost,
    identity: &LocalHostIdentity,
    review_configuration: ReviewConfiguration,
    codes: &launch::LaunchCodes,
    events: &events::ReviewEvents,
    access: Arc<LocalAccess>,
    address: std::net::SocketAddr,
) -> axum::Router {
    let asset_access = access.clone();
    host.router()
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
        .merge(events.router())
        .fallback(move |request: Request| {
            std::future::ready(assets::serve(&asset_access, &request))
        })
        .layer(middleware::from_fn_with_state(
            (access, codes.clone(), format!("ws://{address}")),
            protect_origin,
        ))
}

/// The repository binding and the caller credential, built separately.
fn repository_access(
    options: &Options,
    token: &str,
    address: std::net::SocketAddr,
) -> anyhow::Result<(PathBuf, Arc<LocalRepository>, Arc<LocalAccess>)> {
    let canonical_repo = options
        .repo
        .canonicalize()
        .map_err(|_| OperationFailure::InvalidInput {
            field: None,
            reason: provenance_core::protocol::failure::InvalidInputReason::InvalidValue,
        })
        .context("cannot configure review repository access")?;
    let repository = Arc::new(
        LocalRepository::new(&canonical_repo, &options.repository_id, &options.scope)
            .context("cannot configure review repository access")?,
    );
    let access = Arc::new(
        LocalAccess::new(token, address).context("cannot configure review repository access")?,
    );
    Ok((canonical_repo, repository, access))
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
        codes.issue(std::time::Instant::now(), std::time::SystemTime::now())
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
    mut server: tokio::task::JoinHandle<std::io::Result<()>>,
    stop: tokio::sync::oneshot::Sender<()>,
    host: StatementHost,
    events: events::ReviewEvents,
    mut signals: ShutdownSignals,
) -> anyhow::Result<()> {
    let result = tokio::select! {
        result = &mut server => Some(result),
        () = signals.recv() => {
            let _ = stop.send(());
            None
        }
    };
    events.stop();
    eprintln!("Review host is stopping. Wait for active operations, or send a second signal to force exit; writes may be incomplete.");
    let result = tokio::select! {
        result = async {
            // Join started operations before limiting the remaining HTTP drain.
            host.shutdown().await;
            events.shutdown().await;
            match result {
                Some(result) => result,
                None => tokio::time::timeout(Duration::from_secs(1), &mut server)
                    .await
                    .unwrap_or(Ok(Ok(()))),
            }
        } => result,
        () = signals.recv() => {
            eprintln!("Forced review host exit: writes may be incomplete. Check repository state before retrying a write.");
            // Returning would still wait for blocked tasks when Tokio drops its runtime.
            std::process::exit(1);
        }
    };
    result
        .context("review listener task failed")?
        .context("review listener failed")
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
    credential: Arc<LocalAccess>,
    repository: Arc<LocalRepository>,
    path: camino::Utf8PathBuf,
    value: Value,
}

async fn configuration(
    State(mut config): State<ReviewConfiguration>,
    request: Request,
) -> Response {
    if let Err(error) = config.credential.authenticate(request.headers()) {
        return refusal(error);
    }
    let actor_ids = match config.repository.disposition_actor_ids() {
        Ok(actor_ids) => actor_ids,
        Err(error) => return refusal(error),
    };
    let path = config.path.clone();
    let reviewer = tokio::task::spawn_blocking(move || crate::reviewer::review_actor_id(&path))
        .await
        .ok()
        .flatten();
    if let Some(actor_id) = reviewer {
        config.value["reviewer"] =
            json!({ "allowed": actor_ids.contains(&actor_id), "actorId": actor_id });
    }
    config.value["dispositionActorIds"] = json!(actor_ids);
    Json(config.value).into_response()
}

async fn protect_origin(
    State((access, codes, websocket)): State<(Arc<LocalAccess>, launch::LaunchCodes, String)>,
    mut request: Request,
    next: Next,
) -> Response {
    // Only the static entry document can accept navigation from the private redirect.
    let mut headers = request.headers().clone();
    if matches!(*request.method(), Method::GET | Method::HEAD)
        && matches!(request.uri().path(), "/" | "/index.html")
        && [
            ("sec-fetch-site", "cross-site"),
            ("sec-fetch-mode", "navigate"),
            ("sec-fetch-dest", "document"),
        ]
        .iter()
        .all(|(name, expected)| {
            let mut values = headers.get_all(*name).iter();
            values.next().and_then(|value| value.to_str().ok()) == Some(*expected)
                && values.next().is_none()
        })
    {
        // Keep the exact Host and Origin checks, including duplicate header refusal.
        headers.remove("sec-fetch-site");
    }
    let mut response = match access.check_origin(&headers) {
        Ok(()) => {
            // A cross-site entry navigation serves only assets and cannot use a cookie.
            if access.check_origin(request.headers()).is_ok() {
                let page = codes.authenticate_cookie(request.headers_mut());
                if page && matches!(*request.method(), Method::POST | Method::PATCH) {
                    request
                        .extensions_mut()
                        .insert(provenance_transport::ReviewPageWrite);
                }
            }
            next.run(request).await
        }
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
    ] {
        response
            .headers_mut()
            .insert(name, HeaderValue::from_static(value));
    }
    response.headers_mut().insert(
        "content-security-policy",
        HeaderValue::from_str(&format!("default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self' {websocket}; base-uri 'none'; frame-ancestors 'none'; form-action 'none'; worker-src 'none'"))
            .expect("loopback WebSocket policy"),
    );
    response
}

fn refusal(error: OperationFailure) -> Response {
    (
        StatusCode::from_u16(error.status_code()).expect("operation failure status"),
        Json(FailureEnvelope::new(None, error)),
    )
        .into_response()
}
