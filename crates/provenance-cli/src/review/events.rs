pub(super) mod frames;
mod navigation;
pub(super) mod output;
pub(super) mod stream;
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

    pub fn page_changed(&self, changes: provenance_store::review_changes::ReviewChanges) {
        self.state.subscribers.page_changed(changes);
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
    let headers = request.headers();
    let native = headers.contains_key("x-provenance-launch-key");
    if request.uri().query().is_some() || request.uri().authority().is_some() {
        return super::refusal(OperationFailure::AccessDenied);
    }
    if native {
        // Native credentials cannot be used by a browser, including same-origin scripts.
        if [
            "origin",
            "cookie",
            "authorization",
            "sec-fetch-site",
            "sec-fetch-mode",
            "sec-fetch-dest",
        ]
        .iter()
        .any(|name| headers.contains_key(*name))
        {
            return super::refusal(OperationFailure::AccessDenied);
        }
        let mut keys = headers.get_all("x-provenance-launch-key").iter();
        let key = keys.next().and_then(|value| value.to_str().ok());
        if keys.next().is_some() || !key.is_some_and(|key| state.codes.holds_key(key)) {
            return super::refusal(OperationFailure::Unauthenticated);
        }
        if let Err(error) = state.access.check_origin(headers) {
            return super::refusal(error);
        }
    } else {
        if headers.get_all(header::ORIGIN).iter().count() != 1
            || !metadata(headers, "sec-fetch-mode", "websocket")
            || !metadata(headers, "sec-fetch-dest", "empty")
        {
            return super::refusal(OperationFailure::AccessDenied);
        }
        if headers.get_all(header::AUTHORIZATION).iter().count() > 1 {
            return super::refusal(OperationFailure::Unauthenticated);
        }
        let mut authentication = headers.clone();
        authentication.remove(header::AUTHORIZATION);
        state.codes.authenticate_cookie(&mut authentication);
        if let Err(error) = state.access.authenticate(&authentication) {
            return super::refusal(error);
        }
    }
    let (mut parts, _) = request.into_parts();
    let upgrade = match WebSocketUpgrade::from_request_parts(&mut parts, &()).await {
        Ok(upgrade) => upgrade,
        Err(error) => return error.into_response(),
    };
    let Some(subscription) = state.subscribers.subscribe() else {
        return super::refusal(OperationFailure::UnavailableNeeds);
    };
    let page = (!native).then(|| state.navigation.attach());
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
    mut page: Option<navigation::Page>,
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
                    if let Some(page) = &mut page { page.receive(&text); }
                    continue;
                }
                Some(Ok(_)) => continue,
            },
            Some(link) = async {
                match &mut page {
                    Some(page) => page.links.recv().await,
                    None => std::future::pending().await,
                }
            } => {
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
    frame: &(impl serde::Serialize + Sync),
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
