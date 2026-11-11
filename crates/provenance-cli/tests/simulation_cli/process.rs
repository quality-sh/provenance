//! The `provenance api` and git processes of the CLI simulation.
//!
//! Each wait for a process has the limit [`PROCESS_LIMIT`]. A process that
//! does not end in it is killed, and its case breaks L0, so a stopped
//! process fails its case in minutes and does not hold the job. A process
//! that panics also breaks L0.

use crate::support::api_process;
use crate::support::catch_up_steps::{Answer, Surface};
use provenance_simulation::generate::{
    created_requirement_id, requirement_id, Effect, Operation, Task, FIRST_REQUIREMENT, TOPIC,
};
use provenance_simulation::Done;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::io::{Read, Write as _};
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The longest wait for one process.
pub const PROCESS_LIMIT: Duration = Duration::from_secs(120);

thread_local! {
    /// The processes of this case that did not end in the limit or that
    /// panicked.
    static L0_BREAKS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Takes the processes that did not end in the limit or that panicked since
/// the last call.
pub fn take_l0_breaks() -> Vec<String> {
    L0_BREAKS.with(|breaks| std::mem::take(&mut *breaks.borrow_mut()))
}

fn note_l0_break(text: String) {
    L0_BREAKS.with(|breaks| breaks.borrow_mut().push(text));
}

/// Tells if a process panicked: a Rust panic exits with code 101 and prints
/// "panicked at" on standard error.
fn panicked(output: &Output) -> bool {
    output.status.code() == Some(101)
        || String::from_utf8_lossy(&output.stderr).contains("panicked at")
}

/// The layer name of this system, for example `cli-linux`.
pub fn layer() -> String {
    format!("cli-{}", std::env::consts::OS)
}

pub fn binary() -> &'static Path {
    assert_cmd::cargo::cargo_bin!("provenance")
}

/// Reads a pipe of a child on its own thread.
fn read_pipe(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        bytes
    })
}

/// Waits for `child` for the process limit. At the limit, kills it, notes
/// `what` as an L0 break and refuses. A process that panicked is also an L0
/// break.
pub fn wait_limited(mut child: Child, what: &str) -> Result<Output, String> {
    let stdout = read_pipe(child.stdout.take());
    let stderr = read_pipe(child.stderr.take());
    let deadline = Instant::now() + PROCESS_LIMIT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let text = format!("{what} did not end in {PROCESS_LIMIT:?}");
                note_l0_break(text.clone());
                return Err(text);
            }
            Err(error) => return Err(format!("{what}: {error}")),
        }
    };
    let output = Output {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    };
    if panicked(&output) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        note_l0_break(format!("{what} panicked: {}", stderr.trim()));
    }
    Ok(output)
}

/// Runs `command` to its end, for the process limit at most.
pub fn output(mut command: Command, what: &str) -> Result<Output, String> {
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = command
        .spawn()
        .map_err(|error| format!("{what}: {error}"))?;
    wait_limited(child, what)
}

pub fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = provenance_store::git_process::git();
    command
        .args(["-c", "user.name=Simulation"])
        .args(["-c", "user.email=simulation@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir);
    let output = output(command, &format!("git {args:?}"))?;
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).trim().to_owned();
    if output.status.success() {
        Ok(text(&output.stdout))
    } else {
        Err(format!("git {args:?}: {}", text(&output.stderr)))
    }
}

/// Commits every change in `dir`.
pub fn commit(dir: &Path, message: &str) -> Result<String, String> {
    git(dir, &["add", "-A"])?;
    git(dir, &["commit", "-q", "--allow-empty", "-m", message])
}

fn parse(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap_or(Value::Null)
}

/// The kind of a refusal on standard error, or `no_kind`.
fn refusal_kind(stderr: &str) -> String {
    let error = parse(stderr.trim_start_matches("Error: ").as_bytes());
    error["error"]["kind"]
        .as_str()
        .unwrap_or("no_kind")
        .to_owned()
}

/// The kind at the start of a refusal text: `refused <kind>: ...`.
fn kind_of(text: &str) -> Option<String> {
    let rest = text.strip_prefix("refused ")?;
    rest.split_once(": ").map(|(kind, _)| kind.to_owned())
}

/// The kind of a write that the simulation did not send, because the read
/// of its etag refused.
pub const NOT_SENT: &str = "not_sent";

/// One public API call of a write.
pub struct Request {
    method: &'static str,
    path: String,
    body: Value,
    if_match: Option<String>,
}

impl Request {
    const fn post(path: String, body: Value) -> Self {
        Self {
            method: "post",
            path,
            body,
            if_match: None,
        }
    }

    /// Starts one `provenance api` process.
    fn spawn(&self, root: &Path) -> Child {
        let mut command = Command::new(binary());
        command.args(["api", &self.path, "--format", "json", "--input", "-"]);
        command.args(["--method", self.method]);
        command.arg("--repo").arg(root);
        if let Some(etag) = &self.if_match {
            command.args(["--header", &format!("If-Match: {etag}")]);
        }
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command.spawn().expect("the CLI starts");
        let mut stdin = child.stdin.take().expect("a stdin pipe");
        let body = self.body.to_string();
        stdin.write_all(body.as_bytes()).expect("the CLI reads");
        child
    }
}

/// The data of a success, or the text of a refusal with its kind first.
fn finish(child: Child, what: &str) -> Result<Value, String> {
    let output = wait_limited(child, what)?;
    if output.status.success() {
        return Ok(parse(&output.stdout)["data"].clone());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(format!("refused {}: {stderr}", refusal_kind(&stderr)))
}

/// The `provenance api` reads of the full-rebuild oracle, through the
/// shared command of `support/api_process.rs`, with the process limit.
pub struct Reads(pub String);

impl Surface for Reads {
    fn call<'a>(
        &'a self,
        method: &'a str,
        path: &'a str,
        _: Option<Value>,
        _: &'a [(&'a str, &'a str)],
    ) -> Answer<'a> {
        let command = api_process::command(binary(), &self.0, (method, path), &[], false);
        let answer = match output(command, &format!("read {path}")) {
            Ok(output) => api_process::answer(&output),
            Err(text) => (1, json!(text)),
        };
        Box::pin(async move { answer })
    }
}

/// Polls a future that is ready at once, as each read of [`Reads`] is.
pub fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut context) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("a read waited"),
    }
}

/// The edit etag of a Requirement, read through the CLI.
pub fn etag(root: &Path, id: &str) -> Result<String, String> {
    let path = format!("requirements/{id}");
    let mut command = Command::new(binary());
    command.args(["api", &path, "--format", "json"]);
    command.arg("--repo").arg(root);
    let output = output(command, &format!("read {path}"))?;
    let answer = parse(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let etag = answer["data"]["edit"]["etag"].as_str().map(str::to_owned);
    etag.ok_or_else(|| format!("refused {NOT_SENT}: the etag read refused: {stderr}"))
}

/// The public API call of `task` on the repository at `root`, or `None` when
/// the simulation sends no route for it (thread messages, two bindings in a
/// section, decisions, Discussions and archives) or when there is nothing to
/// act on.
fn request(root: &Path, task: &Task) -> Option<Result<Request, String>> {
    let actor = task.actor();
    let review = format!("requirements/{FIRST_REQUIREMENT}");
    Some(Ok(match &task.op {
        Operation::CreateRequirement { n } => Request::post(
            "requirements".into(),
            json!({"id": created_requirement_id(*n), "statement": "Share links have an owner",
                "status": "active", "depends_on": [], "supersedes": [], "actor": actor}),
        ),
        Operation::UpdateRequirement { n, stale } => {
            let id = requirement_id(*n);
            let etag = if *stale {
                task.stale_etag.clone()
            } else {
                match etag(root, &id) {
                    Ok(etag) => etag,
                    Err(refusal) => return Some(Err(refusal)),
                }
            };
            Request {
                method: "patch",
                path: format!("requirements/{id}"),
                body: json!({"description": task.description(), "actor": actor}),
                if_match: Some(etag),
            }
        }
        Operation::SubmitReview => Request::post(
            format!("{review}/submit"),
            json!({"actor": "reviewer", "title": "Simulated review",
                "summary": format!("Review from operation {}", task.operation),
                "source_ids": [], "evidence_references": [], "builds_on": []}),
        ),
        Operation::WithdrawReview { .. } => {
            let proposal = task.proposal.as_ref()?;
            let path = format!("{review}/submissions/{proposal}/withdraw");
            Request::post(path, json!({"actor": "reviewer"}))
        }
        Operation::ClaimTopic => {
            Request::post(format!("topics/{TOPIC}/claim"), json!({"actor": actor}))
        }
        Operation::ReleaseTopic => {
            Request::post(format!("topics/{TOPIC}/release"), json!({"actor": actor}))
        }
        Operation::PostMessage
        | Operation::BindTwice { .. }
        | Operation::DecideReview { .. }
        | Operation::WriteDiscussion { .. }
        | Operation::ArchiveRequirement { .. } => return None,
    }))
}

/// What a successful CLI write of `task` did.
fn effect(task: &Task, request: &Request, data: &Value) -> Result<Effect, String> {
    let proposal = data["proposal_id"].as_str().map(str::to_owned);
    let proposal = proposal.ok_or_else(|| format!("no proposal in {data}"));
    Ok(match &task.op {
        Operation::CreateRequirement { n } => Effect::Created {
            id: created_requirement_id(*n),
        },
        Operation::UpdateRequirement { n, .. } => Effect::Described {
            id: requirement_id(*n),
            description: task.description(),
            etag: request.if_match.clone().unwrap_or_default(),
        },
        Operation::SubmitReview => Effect::Proposal { id: proposal? },
        Operation::WithdrawReview { .. } => Effect::Withdrawn {
            proposal: proposal?,
        },
        Operation::ClaimTopic => Effect::Claim {
            holder: Some(task.actor()),
        },
        Operation::ReleaseTopic => Effect::Claim { holder: None },
        _ => Effect::Skipped,
    })
}

fn done(task: &Task, result: Result<Effect, String>, stopped: bool) -> Done {
    let kind = result.as_ref().err().and_then(|text| kind_of(text));
    Done {
        engine: task.engine,
        operation: task.operation,
        op: task.op.clone(),
        target: task.target(),
        result,
        kind,
        stopped,
    }
}

/// The total time and the count of the writes that ran to their end.
static WRITE_TIMES: Mutex<(Duration, u32)> = Mutex::new((Duration::ZERO, 0));

/// The mean time of a write process in this test process, or one second
/// before the first write ends.
fn mean_write_time() -> Duration {
    let (total, count) = *WRITE_TIMES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if count == 0 {
        Duration::from_secs(1)
    } else {
        total / count
    }
}

/// When a stop at `at` kills a write: in the second half of the mean write
/// time, where the write publishes.
pub fn stop_delay(at: u8) -> Duration {
    mean_write_time() * (255 + u32::from(at)) / 510
}

/// Kills the write process `child` of `task`. The stop is inside the write
/// when the process runs at the kill and has not printed its answer. A
/// process that ended or printed its answer before the kill gives its
/// answer, as a write alone does.
fn stop_write(task: &Task, mut child: Child, request: &Request) -> Done {
    let ended = |result: Result<Value, String>| {
        provenance_simulation::stats::add(&layer(), "stops after the write ended", 1);
        let result = result.and_then(|data| effect(task, request, &data));
        done(task, result, false)
    };
    if matches!(child.try_wait(), Ok(Some(_))) {
        return ended(finish(child, &format!("write {}", request.path)));
    }
    let _ = child.kill();
    match wait_limited(child, "a stopped write") {
        Ok(output) if !output.stdout.is_empty() => ended(Ok(parse(&output.stdout)["data"].clone())),
        _ => done(task, Err("stopped".into()), true),
    }
}

/// Runs `tasks` at once on `root`. Each etag is read before the first
/// process starts. With `stop`, each process is killed after that delay.
pub fn run_at_once(root: &Path, tasks: &[Task], stop: Option<Duration>) -> Vec<Done> {
    let mut prepared = Vec::new();
    for task in tasks {
        if let Some(request) = request(root, task) {
            prepared.push((task, request));
        }
    }
    let mut started = Vec::new();
    for (task, request) in prepared {
        let process = request.map(|request| (request.spawn(root), request, Instant::now()));
        started.push((task, process));
    }
    let mut outcome = Vec::new();
    for (task, process) in started {
        outcome.push(match (process, stop) {
            (Err(refusal), _) => done(task, Err(refusal), false),
            (Ok((child, request, _)), Some(delay)) => {
                std::thread::sleep(delay);
                stop_write(task, child, &request)
            }
            (Ok((child, request, started)), None) => {
                let result = finish(child, &format!("write {}", request.path));
                if tasks.len() == 1 {
                    let mut times = WRITE_TIMES
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    times.0 += started.elapsed();
                    times.1 += 1;
                }
                let result = result.and_then(|data| effect(task, &request, &data));
                done(task, result, false)
            }
        });
    }
    outcome
}
