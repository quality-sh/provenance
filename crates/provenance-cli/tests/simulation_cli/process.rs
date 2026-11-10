//! The `provenance api` processes of the CLI simulation.

use crate::support::catch_up_steps::{Answer, Surface};
use provenance_simulation::generate::{
    created_requirement_id, requirement_id, Effect, Operation, Task, FIRST_REQUIREMENT, TOPIC,
};
use provenance_simulation::Done;
use serde_json::{json, Value};
use std::io::Write as _;
use std::path::Path;
use std::process::{Child, Command, Stdio};

pub fn binary() -> &'static Path {
    assert_cmd::cargo::cargo_bin!("provenance")
}

pub fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(["-c", "user.name=Simulation"])
        .args(["-c", "user.email=simulation@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|error| format!("{error}"))?;
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
fn finish(child: Child) -> Result<Value, String> {
    let output = child.wait_with_output().expect("the CLI ends");
    if output.status.success() {
        return Ok(parse(&output.stdout)["data"].clone());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let error = parse(stderr.trim_start_matches("Error: ").as_bytes());
    let kind = error["error"]["kind"].as_str().unwrap_or("no_kind");
    Err(format!("refused {kind}: {stderr}"))
}

/// The `provenance api` reads of the full-rebuild oracle.
pub struct Reads(pub String);

impl Surface for Reads {
    fn call<'a>(
        &'a self,
        method: &'a str,
        path: &'a str,
        _: Option<Value>,
        _: &'a [(&'a str, &'a str)],
    ) -> Answer<'a> {
        let path = path.trim_start_matches('/');
        let (path, query) = path.split_once('?').unwrap_or((path, ""));
        let mut command = Command::new(binary());
        command.args(["api", path, "--repo", &self.0, "--format", "json"]);
        command.args(["--method", method]);
        for pair in query.split('&').filter(|pair| !pair.is_empty()) {
            command.args(["--query", pair]);
        }
        let output = command.output().expect("the CLI reads");
        let answer = if output.status.success() {
            (200, parse(&output.stdout))
        } else {
            (1, json!(String::from_utf8_lossy(&output.stderr)))
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
    let output = command.arg("--repo").arg(root).output();
    let output = output.map_err(|error| format!("{error}"))?;
    let answer = parse(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let etag = answer["data"]["edit"]["etag"].as_str().map(str::to_owned);
    etag.ok_or_else(|| format!("refused read: {stderr}"))
}

/// The public API call of `task` on the repository at `root`, or `None` when
/// the API has no route for it (thread messages, two bindings in a section)
/// or when there is nothing to act on.
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
        Operation::PostMessage | Operation::BindTwice { .. } => return None,
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
        Operation::PostMessage | Operation::BindTwice { .. } => Effect::Skipped,
    })
}

fn done(task: &Task, result: Result<Effect, String>, stopped: bool) -> Done {
    Done {
        engine: task.engine,
        operation: task.operation,
        op: task.op.clone(),
        target: task.target(),
        result,
        stopped,
    }
}

/// Runs `tasks` at once on `root`. Each etag is read before the first
/// process starts. With `stop`, each process stops after that many
/// milliseconds.
pub fn run_at_once(root: &Path, tasks: &[Task], stop: Option<u64>) -> Vec<Done> {
    let mut prepared = Vec::new();
    for task in tasks {
        if let Some(request) = request(root, task) {
            prepared.push((task, request));
        }
    }
    let mut started = Vec::new();
    for (task, request) in prepared {
        let process = request.map(|request| (request.spawn(root), request));
        started.push((task, process));
    }
    let mut outcome = Vec::new();
    for (task, process) in started {
        outcome.push(match (process, stop) {
            (Err(refusal), _) => done(task, Err(refusal), false),
            (Ok((mut child, _)), Some(millis)) => {
                std::thread::sleep(std::time::Duration::from_millis(millis));
                let _ = child.kill();
                let _ = child.wait();
                done(task, Err("stopped".into()), true)
            }
            (Ok((child, request)), None) => {
                let result = finish(child);
                let result = result.and_then(|data| effect(task, &request, &data));
                done(task, result, false)
            }
        });
    }
    outcome
}
