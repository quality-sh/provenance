mod frames;
mod subscribers;
mod watch;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        FromRequestParts, Request, State, WebSocketUpgrade,
    },
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    routing::get,
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
                access,
                codes,
            },
            observer: Arc::new(Mutex::new(observer)),
        }
    }

    pub fn router(&self) -> axum::Router {
        axum::Router::new()
            .route("/review-events", get(upgrade))
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
    upgrade
        .on_upgrade(move |socket| connected(socket, subscription))
        .into_response()
}

fn metadata(headers: &HeaderMap, name: &str, expected: &str) -> bool {
    if !headers.contains_key(name) {
        return true;
    }
    let mut values = headers.get_all(name).iter();
    values.next().and_then(|value| value.to_str().ok()) == Some(expected) && values.next().is_none()
}

async fn connected(mut socket: WebSocket, mut subscription: Subscription) {
    if !send(&mut socket, &subscription.ready, &mut subscription.stopped).await {
        return;
    }
    loop {
        let delivery = tokio::select! {
            frame = subscription.receiver.recv() => frame,
            incoming = socket.recv() => {
                if matches!(incoming, None | Some(Err(_) | Ok(Message::Close(_)))) { break; }
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
            Ok(None) | Err(_) => break,
        }
    }
    let _ = socket.send(Message::Close(None)).await;
}

async fn send(
    socket: &mut WebSocket,
    frame: &frames::Frame,
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
