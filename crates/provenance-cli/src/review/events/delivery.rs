//! Host-side review notifications. The host checks a registration, queues the full document
//! into the Codex thread with `codex queue`, and then queues one message for each follow frame.
//! Codex accepting a message into the thread queue does not show that the agent read it.
use super::{
    envelope::{self, Selection},
    frames::Frame,
    output::OutputFrame,
    stream::Follower,
    subscribers::{Subscribers, Subscription},
};
use axum::{
    response::{IntoResponse, Response},
    Json,
};
use provenance_cli::repo_context::RepoContext;
use provenance_macros::rule;
use provenance_porcelain::get::{Notify, NotifyClient, Registration};
use serde::Deserialize;
use std::{
    collections::HashMap,
    io::{BufRead as _, BufReader, Read as _, Write as _},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};
use tokio::sync::{broadcast::error::RecvError, watch, Mutex};

const CODEX_TIMEOUT: Duration = Duration::from_secs(60);
const SHUTDOWN_WAIT: Duration = Duration::from_secs(30);

/// One notify request from a CLI or MCP caller that holds the launch key.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DeliveryRequest {
    repository_id: String,
    scope: String,
    instance_nonce: String,
    root_id: String,
    limit: usize,
    notify: Notify,
    codex_home: Option<String>,
}

/// A refused registration names the request field to change; a failed one has no field.
pub(super) struct Refusal {
    field: Option<&'static str>,
    message: String,
}

impl Refusal {
    fn input(field: &'static str, message: &str) -> Self {
        Self {
            field: Some(field),
            message: message.to_owned(),
        }
    }

    const fn failed(message: String) -> Self {
        Self {
            field: None,
            message,
        }
    }
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        let (status, error) = match self.field {
            Some(field) => (
                axum::http::StatusCode::BAD_REQUEST,
                serde_json::json!({"kind": "invalid_input", "field": field,
                    "reason": "invalid_value", "message": self.message}),
            ),
            None => (
                axum::http::StatusCode::BAD_GATEWAY,
                serde_json::json!({"kind": "operation_failed", "message": self.message}),
            ),
        };
        (
            status,
            Json(serde_json::json!({"error": error, "meta": {}})),
        )
            .into_response()
    }
}

#[derive(Clone)]
pub(super) struct Deliveries {
    inner: Arc<Inner>,
}

struct Inner {
    root: PathBuf,
    context: RepoContext,
    epoch: String,
    codex_home: Option<PathBuf>,
    subscribers: Subscribers,
    // Registrations and removals run one at a time, so a thread has at most one task.
    threads: Mutex<HashMap<String, Active>>,
}

struct Active {
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}

/// The registered thread and the selection of its messages.
struct Target {
    thread: String,
    repository: String,
    root: String,
    limit: usize,
}

impl Deliveries {
    pub fn new(root: &Path, scope: &str, epoch: &str, subscribers: Subscribers) -> Self {
        let root = dunce::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let context = RepoContext::new(
            camino::Utf8PathBuf::from_path_buf(root.clone())
                .unwrap_or_else(|path| path.to_string_lossy().into_owned().into()),
            scope,
        );
        Self {
            inner: Arc::new(Inner {
                root,
                context,
                epoch: epoch.to_owned(),
                codex_home: provenance_cli::notify::codex_home(),
                subscribers,
                threads: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Registers a thread after the full document is in its queue, or removes the
    /// registration of the thread for `off`.
    #[rule("rule_review_notify_confirms_first")]
    pub async fn register(&self, request: DeliveryRequest) -> Result<Registration, Refusal> {
        let inner = &self.inner;
        let repository = inner.check(&request).await?;
        let thread = request.notify.thread.clone();
        let mut threads = inner.threads.lock().await;
        if let Some(active) = threads.remove(&thread) {
            active.end().await;
        }
        if request.notify.client == NotifyClient::Off {
            drop(threads);
            return Ok(Registration {
                client: NotifyClient::Off,
                thread,
                registered: false,
                accepted: false,
                removed: true,
            });
        }
        let home = inner.codex_home.clone();
        let read = thread.clone();
        let cwd = tokio::task::spawn_blocking(move || thread_cwd(home.as_deref(), &read))
            .await
            .map_err(|error| Refusal::failed(error.to_string()))?
            .map_err(|_| Refusal::input("notify.thread", "Codex has no thread with this ID"))?;
        if !dunce::canonicalize(&cwd).is_ok_and(|cwd| cwd.starts_with(&inner.root)) {
            return Err(Refusal::input(
                "notify.thread",
                "the Codex thread does not run in this repository",
            ));
        }
        let Some(subscription) = inner.subscribers.subscribe(true) else {
            return Err(Refusal::failed(
                "live review updates are unavailable".to_owned(),
            ));
        };
        let target = Target {
            thread: thread.clone(),
            repository,
            root: request.root_id,
            limit: request.limit.max(1),
        };
        let mut follower = Follower::new(&inner.epoch);
        let ready = subscription.ready.clone();
        let (_, at) = ready.position();
        let frames = follower
            .start(&inner.context, &target.root, target.limit, &ready, || {
                inner.subscribers.native_sequence() != at
            })
            .await
            .map_err(|error| Refusal::failed(format!("{error:#}")))?;
        let mut refused = None;
        for frame in &frames {
            if let Err(error) = inner.queue(&target, frame).await {
                refused = Some(Refusal::failed(format!(
                    "Codex did not accept the message: {error:#}"
                )));
                break;
            }
        }
        let (stop, stopped) = watch::channel(false);
        let task = tokio::spawn(deliver(
            self.inner.clone(),
            target,
            subscription,
            follower,
            stopped,
        ));
        threads.insert(thread.clone(), Active { stop, task });
        drop(threads);
        if let Some(refused) = refused {
            return Err(refused);
        }
        Ok(Registration {
            client: NotifyClient::Codex,
            thread,
            registered: true,
            accepted: true,
            removed: false,
        })
    }

    /// Waits for each delivery task to queue its disconnected message after the host stops.
    pub async fn shutdown(&self) {
        let tasks = std::mem::take(&mut *self.inner.threads.lock().await);
        for (_, active) in tasks {
            let _ = tokio::time::timeout(SHUTDOWN_WAIT, active.task).await;
        }
    }
}

impl Active {
    /// Stops the task without a message, so a replaced or removed registration queues nothing.
    async fn end(self) {
        self.stop.send_replace(true);
        let _ = self.task.await;
    }
}

impl Inner {
    /// Returns the repository ID after the host identity, thread and Codex home checks.
    #[rule("rule_review_notify_checks_target")]
    async fn check(&self, request: &DeliveryRequest) -> Result<String, Refusal> {
        if request.scope != self.context.scope {
            return Err(Refusal::input(
                "scope",
                "the review host serves another scope",
            ));
        }
        if request.instance_nonce != self.epoch {
            return Err(Refusal::input(
                "instanceNonce",
                "the request names another review host instance",
            ));
        }
        let root = self.root.clone();
        let scope = self.context.scope.clone();
        let own = tokio::task::spawn_blocking(move || {
            provenance_transport::local_host::discover(&root, &scope)
        })
        .await
        .map_err(|error| Refusal::failed(error.to_string()))?
        .map_err(|error| Refusal::failed(format!("{error:#}")))?
        .filter(|host| host.instance_nonce() == self.epoch)
        .ok_or_else(|| Refusal::failed("the review host has no published location".to_owned()))?;
        if request.repository_id != own.repository_id() {
            return Err(Refusal::input(
                "repositoryId",
                "the review host serves another repository",
            ));
        }
        if uuid::Uuid::parse_str(&request.notify.thread).is_err() {
            return Err(Refusal::input(
                "notify.thread",
                "the Codex thread is not a UUID",
            ));
        }
        let requested = request.codex_home.as_deref().map(Path::new);
        if requested.map(same_path) != self.codex_home.as_deref().map(same_path) {
            return Err(Refusal::input(
                "codexHome",
                "the Codex home is not the Codex home of the review host",
            ));
        }
        Ok(own.repository_id().to_owned())
    }

    /// Queues one frame into the thread. Exit status 0 means Codex accepted the message.
    #[rule("rule_review_notify_codex_queue")]
    async fn queue(&self, target: &Target, frame: &OutputFrame) -> anyhow::Result<()> {
        let selection = Selection {
            repository: &target.repository,
            scope: &self.context.scope,
            root: &target.root,
        };
        let message = envelope::message(&selection, frame)?;
        let home = self.codex_home.clone();
        let thread = target.thread.clone();
        tokio::task::spawn_blocking(move || queue(home.as_deref(), &thread, &message)).await?
    }
}

/// Queues each follow frame until the registration ends. The host stop queues a
/// disconnected message; a queue failure ends the registration with one stderr line.
async fn deliver(
    inner: Arc<Inner>,
    target: Target,
    mut subscription: Subscription,
    mut follower: Follower,
    mut stop: watch::Receiver<bool>,
) {
    loop {
        let frame = match next(&mut subscription, &mut stop, &follower).await {
            Next::Frame(frame) => frame,
            Next::Stop => return,
            Next::Disconnected => {
                let (epoch, sequence) = follower.position();
                let end = OutputFrame::disconnected(epoch, sequence);
                let _ = queue_or_report(&inner, &target, &end).await;
                return;
            }
        };
        match read(&inner, &target, &mut follower, &frame).await {
            Ok(frames) => {
                for frame in &frames {
                    if queue_or_report(&inner, &target, frame).await.is_err() {
                        return;
                    }
                }
            }
            Err(failure) => {
                let _ = queue_or_report(&inner, &target, &failure).await;
                return;
            }
        }
    }
}

enum Next {
    Frame(Frame),
    Stop,
    Disconnected,
}

/// Waits for the next native frame. A lost frame becomes a reset, so the next read is full.
async fn next(
    subscription: &mut Subscription,
    stop: &mut watch::Receiver<bool>,
    follower: &Follower,
) -> Next {
    let received = tokio::select! {
        frame = subscription.receiver.recv() => frame,
        _ = stop.changed() => return Next::Stop,
        _ = subscription.stopped.changed() => Ok(None),
    };
    match received {
        Ok(Some(frame)) => Next::Frame(frame),
        Err(RecvError::Lagged(_)) => {
            let (epoch, sequence) = follower.position();
            Next::Frame(Frame::Reset {
                epoch,
                sequence: sequence + 1,
                reason: "subscriber-loss".to_owned(),
            })
        }
        Ok(None) | Err(RecvError::Closed) => Next::Disconnected,
    }
}

/// Reads current state for one frame. A page write between two document pages makes the
/// later page refuse, so a failed read runs once more; that read sees no next sequence and
/// delivers a full document.
async fn read(
    inner: &Inner,
    target: &Target,
    follower: &mut Follower,
    frame: &Frame,
) -> Result<Vec<OutputFrame>, OutputFrame> {
    let mut read = follower
        .step(&inner.context, &target.root, target.limit, frame)
        .await;
    if read.is_err() {
        read = follower
            .step(&inner.context, &target.root, target.limit, frame)
            .await;
    }
    read.map_err(|error| {
        let (epoch, sequence) = follower.position();
        OutputFrame::read_error(epoch, sequence, error.to_string())
    })
}

async fn queue_or_report(inner: &Inner, target: &Target, frame: &OutputFrame) -> Result<(), ()> {
    inner.queue(target, frame).await.map_err(|error| {
        eprintln!(
            "Review notifications for Codex thread {} stopped: {error:#}",
            target.thread
        );
    })
}

fn same_path(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn codex(home: Option<&Path>) -> Command {
    let mut command = Command::new("codex");
    if let Some(home) = home {
        command.env("CODEX_HOME", home);
    }
    command
}

/// Runs `codex queue` for one message. It passes only `--thread` and `--message`.
fn queue(home: Option<&Path>, thread: &str, message: &str) -> anyhow::Result<()> {
    let mut child = codex(home)
        .args(["queue", "--thread", thread, "--message", message])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| anyhow::anyhow!("cannot start codex: {error}"))?;
    let mut stderr = child.stderr.take().expect("piped codex stderr");
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    let deadline = Instant::now() + CODEX_TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!(
                "codex queue did not finish in {} seconds",
                CODEX_TIMEOUT.as_secs()
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let errors = errors.join().unwrap_or_default();
    anyhow::ensure!(
        status.success(),
        "codex queue failed: {}",
        errors
            .lines()
            .rfind(|line| !line.trim().is_empty())
            .unwrap_or("no message")
    );
    Ok(())
}

/// Reads the working directory of a thread through a stdio `codex app-server`.
fn thread_cwd(home: Option<&Path>, thread: &str) -> anyhow::Result<PathBuf> {
    let mut child = codex(home)
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = child.stdout.take().expect("piped app server stdout");
    let (send, lines) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if line.map(|line| send.send(line)).is_err() {
                break;
            }
        }
    });
    let mut stdin = child.stdin.take().expect("piped app server stdin");
    let requests = [
        serde_json::json!({"id": 1, "method": "initialize", "params": {
            "clientInfo": {"name": "provenance", "version": env!("CARGO_PKG_VERSION")},
            "capabilities": {"experimentalApi": true}}}),
        serde_json::json!({"method": "initialized"}),
        serde_json::json!({"id": 2, "method": "thread/read",
            "params": {"threadId": thread, "includeTurns": false}}),
    ];
    let written = requests
        .iter()
        .try_for_each(|request| writeln!(stdin, "{request}"));
    let deadline = Instant::now() + CODEX_TIMEOUT;
    let reply = written.map_err(anyhow::Error::from).and_then(|()| loop {
        let line = lines.recv_timeout(deadline.saturating_duration_since(Instant::now()))?;
        let value: serde_json::Value = serde_json::from_str(&line).unwrap_or_default();
        if value["id"] == 2 {
            break Ok(value);
        }
    });
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    let reply = reply?;
    let cwd = reply["result"]["thread"]["cwd"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Codex did not return the thread"))?;
    Ok(PathBuf::from(cwd))
}
