//! The review host: a loopback listener that serves the review page, the repository
//! operations and the live review events for one repository and scope.
//!
//! `start` binds the listener and announces the host; `StartedHost::serve` runs it until the
//! stop future completes, then drains. The application that calls them owns the process:
//! it installs signals, writes to its terminal and opens the browser.

mod connection;
mod events;
pub mod follow;
mod launch;

use crate::{reviewer, RepositoryScope, LAUNCH_CODE_ROUTE, LAUNCH_SESSION_ROUTE};
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
use provenance_porcelain::check::CheckPort;
use provenance_store::{operations::LocalRepository, repository::Repository};
use provenance_transport::{
    local_host::{LocalHostIdentity, LocalHostRegistration, IDENTITY_ROUTE},
    HttpCredential, LocalAccess, StatementHost,
};
use serde_json::{json, Value};
use std::{
    future::{Future, IntoFuture},
    sync::Arc,
    time::Duration,
};
use url::Url;

/// What a review host serves and where it listens.
pub struct HostOptions {
    /// The repository root and the declared scope.
    pub place: RepositoryScope,
    /// The opaque repository target that the browser sees.
    pub repository_id: String,
    /// The loopback port. Zero selects an available port.
    pub port: u16,
    /// The repository-aware computations of the `check` operation.
    pub check_port: Arc<dyn CheckPort>,
}

/// Where a started host listens and the credential that its operations accept.
#[derive(Clone, Debug)]
pub struct Startup {
    endpoint: String,
    bearer: String,
    repository_id: String,
    scope: String,
    instance_nonce: String,
}

impl Startup {
    /// The `http://127.0.0.1:<port>` base of the host.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// The bearer token that the repository operations accept.
    pub fn bearer(&self) -> &str {
        &self.bearer
    }

    pub fn repository_id(&self) -> &str {
        &self.repository_id
    }

    pub fn scope(&self) -> &str {
        &self.scope
    }

    /// The name of this host instance in the local host registry.
    pub fn instance_nonce(&self) -> &str {
        &self.instance_nonce
    }

    /// The review page, without a launch code.
    pub fn url(&self) -> String {
        format!("{}/", self.endpoint)
    }
}

/// A review host that listens and is announced, before it serves.
pub struct StartedHost {
    startup: Startup,
    has_reviewer: bool,
    codes: launch::LaunchCodes,
    events: events::ReviewEvents,
    host: StatementHost,
    server: tokio::task::JoinHandle<std::io::Result<()>>,
    stop: tokio::sync::oneshot::Sender<()>,
    // The registry record names this instance until the host drops it.
    _registration: LocalHostRegistration,
}

/// Binds the loopback listener, opens the repository, starts the live events and announces
/// the host. The host answers requests from this point.
pub async fn start(options: HostOptions) -> anyhow::Result<StartedHost> {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, options.port))
        .await
        .context("cannot bind review listener")?;
    let address = listener.local_addr()?;
    let token = launch::secret();
    let (repository, binding, access) = repository_access(&options, &token, address)?;
    let canonical_repo = repository.working_tree().as_std_path().to_path_buf();
    let has_reviewer = !binding.disposition_actor_ids()?.is_empty();
    let host = StatementHost::with_access(binding.clone())
        .with_credential(access.clone())
        .with_check_port(options.check_port);
    let endpoint = format!("http://{address}");
    // Discovery treats a registered host that does not answer as stale, so the record is
    // announced only after the listener serves.
    let registration = LocalHostRegistration::prepare(
        &canonical_repo,
        &options.place.scope,
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
        repository.clone(),
        options.place.scope.clone(),
        identity.instance_nonce.clone(),
        access.clone(),
        codes.clone(),
    )
    .await;
    let config = json!({
        "endpoint": endpoint, "repositoryId": options.repository_id, "scope": options.place.scope,
        "compatibility": provenance_core::protocol::host::COMPATIBILITY,
        "sdkVersion": env!("CARGO_PKG_VERSION"),
    });
    let review_configuration = ReviewConfiguration {
        credential: access.clone(),
        repository: binding,
        path: repository.working_tree().to_path_buf(),
        value: config,
    };
    let page_events = events.clone();
    let host = host.with_page_writes(
        repository,
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
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(
        axum::serve(connection::Listener(listener), router)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .into_future(),
    );
    tokio::task::yield_now().await;
    registration.announce()?;
    Ok(StartedHost {
        startup: Startup {
            endpoint,
            bearer: token,
            repository_id: options.repository_id,
            scope: options.place.scope,
            instance_nonce: identity.instance_nonce,
        },
        has_reviewer,
        codes,
        events,
        host,
        server,
        stop,
        _registration: registration,
    })
}

impl StartedHost {
    pub const fn startup(&self) -> &Startup {
        &self.startup
    }

    /// Whether the repository names a reviewer; without one the review page is read-only.
    pub const fn has_reviewer(&self) -> bool {
        self.has_reviewer
    }

    /// The review page with a single-use launch code that signs the page in.
    pub fn launch_link(&self) -> Url {
        let mut link = Url::parse(&self.startup.url()).expect("loopback review page URL");
        link.set_fragment(Some(&format!(
            "launch={}",
            self.codes
                .issue(std::time::Instant::now(), std::time::SystemTime::now())
        )));
        link
    }

    /// Serves until `stop` completes or the listener fails, then drains: `stopping` runs
    /// once at the start of the drain, started operations finish, the live events end, and
    /// the remaining HTTP connections get one second. The registry record goes with the host.
    pub async fn serve(
        self,
        stop: impl Future<Output = ()> + Send,
        stopping: impl FnOnce() + Send,
    ) -> anyhow::Result<()> {
        // The codes hold the launch key file and the registration holds the registry record;
        // both go when this function returns.
        let Self {
            codes: _codes,
            events,
            host,
            mut server,
            stop: stop_server,
            _registration,
            ..
        } = self;
        let result = tokio::select! {
            result = &mut server => Some(result),
            () = stop => {
                let _ = stop_server.send(());
                None
            }
        };
        events.stop();
        stopping();
        // Join started operations before limiting the remaining HTTP drain.
        host.shutdown().await;
        events.shutdown().await;
        let result = match result {
            Some(result) => result,
            None => tokio::time::timeout(Duration::from_secs(1), &mut server)
                .await
                .unwrap_or(Ok(Ok(()))),
        };
        result
            .context("review listener task failed")?
            .context("review listener failed")
    }
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
            std::future::ready(crate::assets::serve(&asset_access, &request))
        })
        .layer(middleware::from_fn_with_state(
            (access, codes.clone(), format!("ws://{address}")),
            protect_origin,
        ))
}

/// The opened repository, its alias binding and the caller credential, built separately.
fn repository_access(
    options: &HostOptions,
    token: &str,
    address: std::net::SocketAddr,
) -> anyhow::Result<(Repository, Arc<LocalRepository>, Arc<LocalAccess>)> {
    let repository = Repository::open(&options.place.root, &options.place.scope)
        .context("cannot configure review repository access")?;
    let binding = Arc::new(
        LocalRepository::bind(repository.clone(), &options.repository_id)
            .context("cannot configure review repository access")?,
    );
    let access = Arc::new(
        LocalAccess::new(token, address).context("cannot configure review repository access")?,
    );
    Ok((repository, binding, access))
}

async fn local_host_identity(State(identity): State<LocalHostIdentity>) -> Json<LocalHostIdentity> {
    Json(identity)
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
    let actor_ids = match disposition_actor_ids(config.repository.clone()).await {
        Ok(actor_ids) => actor_ids,
        Err(error) => return refusal(error),
    };
    let path = config.path.clone();
    let reviewer = tokio::task::spawn_blocking(move || reviewer::review_actor_id(&path))
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

/// The manifest read can wait for the publication lock, so it runs off the runtime workers.
async fn disposition_actor_ids(
    repository: Arc<LocalRepository>,
) -> Result<Vec<String>, OperationFailure> {
    tokio::task::spawn_blocking(move || repository.disposition_actor_ids())
        .await
        .unwrap_or(Err(OperationFailure::Internal))
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
            // A cross-site entry navigation serves only assets and cannot use a page session.
            if access.check_origin(request.headers()).is_ok() {
                let page = codes.authenticate_session(request.headers_mut());
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
