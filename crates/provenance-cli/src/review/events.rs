mod frames;
mod navigation;
mod subscribers;
mod watch;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        FromRequestParts, Request, State, WebSocketUpgrade,
    },
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json,
};
use provenance_core::protocol::failure::OperationFailure;
use provenance_store::layout::ProvenanceLayout;
use provenance_transport::{HostAccess, LocalAccess};
use std::sync::Arc;
use subscribers::{Subscribers, Subscription};
use tokio::sync::{watch as stop_watch, Mutex};

#[derive(Clone)]
struct RouteState {
    subscribers: Subscribers,
    navigation: navigation::Navigation,
    access: Arc<LocalAccess>,
    codes: super::launch::LaunchCodes,
}

#[derive(Clone)]
pub(super) struct ReviewEvents {
    state: RouteState,
    observer: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl ReviewEvents {
    pub async fn start(
        layout: ProvenanceLayout,
        scope: String,
        epoch: String,
        access: Arc<LocalAccess>,
        codes: super::launch::LaunchCodes,
    ) -> Self {
        let subscribers = Subscribers::new(epoch, scope);
        let observation =
            tokio::task::spawn_blocking(move || watch::Observation::new(layout)).await;
        let observer = match observation {
            Ok(Ok(observation)) => {
                let baseline = match observation.read(false).await {
                    Ok(baseline) => {
                        subscribers.available();
                        Some(baseline)
                    }
                    Err(error) => {
                        eprintln!("Live review updates are unavailable: {error:#}");
                        None
                    }
                };
                Some(tokio::spawn(observation.run(baseline, subscribers.clone())))
            }
            Ok(Err(error)) => {
                eprintln!("Live review updates are unavailable: {error:#}");
                None
            }
            Err(error) => {
                eprintln!("Live review updates are unavailable: {error}");
                None
            }
        };
        Self {
            state: RouteState {
                subscribers,
                navigation: navigation::Navigation::default(),
                access,
                codes,
            },
            observer: Arc::new(Mutex::new(observer)),
        }
    }

    pub fn router(&self) -> axum::Router {
        axum::Router::new()
            .route("/review-events", get(upgrade))
            .route("/review-navigation", post(navigate))
            .with_state(self.state.clone())
    }

    pub fn stop(&self) {
        self.state.subscribers.stop();
    }

    pub async fn shutdown(&self) {
        self.stop();
        let observer = self.observer.lock().await.take();
        if let Some(observer) = observer {
            if let Err(error) = observer.await {
                eprintln!("Live review observation stopped with an error: {error}");
            }
        }
    }
}

async fn upgrade(State(state): State<RouteState>, request: Request) -> Response {
    if request.uri().query().is_some()
        || request.uri().authority().is_some()
        || request.headers().get_all(header::ORIGIN).iter().count() != 1
        || !metadata(request.headers(), "sec-fetch-mode", "websocket")
        || !metadata(request.headers(), "sec-fetch-dest", "empty")
    {
        return super::refusal(OperationFailure::AccessDenied);
    }
    if request
        .headers()
        .get_all(header::AUTHORIZATION)
        .iter()
        .count()
        > 1
    {
        return super::refusal(OperationFailure::Unauthenticated);
    }
    // This route accepts the host session cookie, without a bearer fallback.
    let mut authentication = request.headers().clone();
    authentication.remove(header::AUTHORIZATION);
    state.codes.authenticate_cookie(&mut authentication);
    if let Err(error) = state.access.authenticate(&authentication) {
        return super::refusal(error);
    }
    let (mut parts, _) = request.into_parts();
    let upgrade = match WebSocketUpgrade::from_request_parts(&mut parts, &()).await {
        Ok(upgrade) => upgrade,
        Err(error) => return error.into_response(),
    };
    let Some(subscription) = state.subscribers.subscribe() else {
        return super::refusal(OperationFailure::UnavailableNeeds);
    };
    let page = state.navigation.attach();
    upgrade
        .on_upgrade(move |socket| connected(socket, subscription, page, state.subscribers))
        .into_response()
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NavigationRequest {
    launch_key: String,
    root_id: String,
    focus_id: Option<String>,
}

/// Lets the launch key holder show a record in a connected page; the reply waits for its
/// acknowledgement, a disconnect or the caller's cancellation.
async fn navigate(State(state): State<RouteState>, body: axum::body::Bytes) -> Response {
    match serde_json::from_slice::<NavigationRequest>(&body) {
        Ok(request) if state.codes.holds_key(&request.launch_key) => {
            let shown = state
                .navigation
                .show(request.root_id, request.focus_id)
                .await;
            Json(serde_json::json!({ "shown": shown })).into_response()
        }
        _ => super::refusal(OperationFailure::Unauthenticated),
    }
}

fn metadata(headers: &HeaderMap, name: &str, expected: &str) -> bool {
    if !headers.contains_key(name) {
        return true;
    }
    let mut values = headers.get_all(name).iter();
    values.next().and_then(|value| value.to_str().ok()) == Some(expected) && values.next().is_none()
}

async fn connected(
    mut socket: WebSocket,
    mut subscription: Subscription,
    mut page: navigation::Page,
    subscribers: Subscribers,
) {
    if !send(&mut socket, &subscription.ready, &mut subscription.stopped).await {
        return;
    }
    loop {
        let delivery = tokio::select! {
            frame = subscription.receiver.recv() => frame,
            incoming = socket.recv() => match incoming {
                None | Some(Err(_) | Ok(Message::Close(_))) => break,
                Some(Ok(Message::Text(text))) => {
                    page.receive(&text);
                    continue;
                }
                Some(Ok(_)) => continue,
            },
            Some(link) = page.links.recv() => {
                if !send(&mut socket, &link, &mut subscription.stopped).await {
                    break;
                }
                continue;
            }
            _ = subscription.stopped.changed() => break,
        };
        match delivery {
            Ok(Some(frame)) => {
                if !send(&mut socket, &frame, &mut subscription.stopped).await {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                let reset = subscribers.reset("subscriber-loss");
                let _ = send(&mut socket, &reset, &mut subscription.stopped).await;
                break;
            }
            Ok(None) | Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
    let _ = socket.send(Message::Close(None)).await;
}

async fn send(
    socket: &mut WebSocket,
    frame: &impl serde::Serialize,
    stopped: &mut stop_watch::Receiver<bool>,
) -> bool {
    if *stopped.borrow() {
        return false;
    }
    tokio::select! {
        result = socket.send(Message::Text(serde_json::to_string(frame).expect("review frame serialization"))) => result.is_ok(),
        _ = stopped.changed() => false,
    }
}
