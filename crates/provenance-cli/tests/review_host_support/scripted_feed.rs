//! Independent host peer for the public CLI's continuity and protocol error checks.
use super::{cache_directory, follow::Follow, use_user_cache};
use axum::{
    extract::ws::{Message, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use provenance_transport::local_host::{LocalHostRegistration, IDENTITY_ROUTE};
use serde_json::Value;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, oneshot};

pub struct Feed {
    pub epoch: String,
    sender: mpsc::UnboundedSender<Value>,
    stop: Option<oneshot::Sender<()>>,
    worker: Option<std::thread::JoinHandle<()>>,
    _registration: LocalHostRegistration,
    cache: tempfile::TempDir,
}

impl Feed {
    pub fn start(root: &Path) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let registration =
            LocalHostRegistration::publish(root, "default", &endpoint, "fixture").unwrap();
        let identity = registration.identity();
        let epoch = identity.instance_nonce.clone();
        let cache = tempfile::tempdir().unwrap();
        let directory = cache_directory(cache.path()).join("provenance/review-launch");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!("{epoch}.key"));
        let key = "a".repeat(64);
        std::fs::write(&path, &key).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let (sender, receiver) = mpsc::unbounded_channel::<Value>();
        let receiver = Arc::new(Mutex::new(Some(receiver)));
        let (stop, stopped) = oneshot::channel();
        let worker = std::thread::spawn(move || {
            tokio::runtime::Runtime::new().unwrap().block_on(async move {
                let app = Router::new()
                    .route(IDENTITY_ROUTE, get(move || { let identity = identity.clone(); async move { Json(identity) } }))
                    .route("/review-events", get(move |headers: axum::http::HeaderMap, upgrade: WebSocketUpgrade| {
                        assert_eq!(headers["x-provenance-launch-key"], key);
                        let mut frames = receiver.lock().unwrap().take().unwrap();
                        async move { upgrade.on_upgrade(move |mut socket| async move {
                            loop {
                                tokio::select! {
                                    frame = frames.recv() => match frame {
                                        Some(frame) => if socket.send(Message::Text(frame.to_string())).await.is_err() { break; },
                                        None => break,
                                    },
                                    incoming = socket.recv() => if matches!(incoming, None | Some(Err(_) | Ok(Message::Close(_)))) { break; },
                                }
                            }
                        }).into_response() }
                    }));
                axum::serve(tokio::net::TcpListener::from_std(listener).unwrap(), app)
                    .with_graceful_shutdown(async { let _ = stopped.await; }).await.unwrap();
            });
        });
        Self {
            epoch,
            sender,
            stop: Some(stop),
            worker: Some(worker),
            _registration: registration,
            cache,
        }
    }
    pub fn send(&self, frame: Value) {
        self.sender.send(frame).unwrap();
    }
    pub fn follow(&self, root: &Path) -> Follow {
        let mut command = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"));
        use_user_cache(&mut command, self.cache.path());
        command.args([
            "req_follow",
            "get",
            "--view",
            "review",
            "--follow",
            "--format",
            "json",
            "--repo",
            root.to_str().unwrap(),
        ]);
        Follow::spawn(command)
    }
}
impl Drop for Feed {
    fn drop(&mut self) {
        let _ = self.stop.take().unwrap().send(());
        self.worker.take().unwrap().join().unwrap();
    }
}
