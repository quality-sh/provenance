//! Implementation aid for bead provenance-ajn2, on a wip branch only; it has no Rule
//! binding. It records the answer of each write with and without an actor, on a
//! spawned `provenance mcp` and on the CLI, in one fixture repository. Each answer
//! goes to standard error on one line that starts with `AJN2 `.

use provenance_mcp::Client;
use serde_json::{json, Value};
use std::process::Stdio;

/// The exit code, standard output and standard error of one CLI command.
fn run(args: &[&str]) -> Value {
    let output = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(args)
        .output()
        .unwrap();
    json!({
        "exit": output.status.code(),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
    })
}

/// Runs one CLI command that must succeed, and gives its standard output.
fn ok(args: &[&str]) -> String {
    let answer = run(args);
    assert_eq!(answer["exit"], json!(0), "{args:?}: {answer}");
    answer["stdout"].as_str().unwrap().to_owned()
}

/// The edit ETag of one record, as the CLI review view reads it.
fn etag(repo: &str, id: &str) -> String {
    let read = ok(&[id, "get", "--view", "review", "--repo", repo, "--format", "json"]);
    let value: Value = serde_json::from_str(&read).unwrap();
    value["review"]["edit"]["etag"]
        .as_str()
        .unwrap_or_else(|| panic!("{id}: {value}"))
        .to_owned()
}

/// The decision state of one record, as the CLI review view reads it.
fn decision(repo: &str, id: &str) -> Value {
    let read = ok(&[id, "get", "--view", "review", "--repo", repo, "--format", "json"]);
    let value: Value = serde_json::from_str(&read).unwrap();
    value["review"]["decision"].clone()
}

/// A short form of one answer: ok, or the refusal kind, field and reason.
fn outcome(answer: &Value) -> String {
    if let Some(exit) = answer.get("exit") {
        if exit == &json!(0) {
            return "ok".into();
        }
        let stderr = answer["stderr"].as_str().unwrap_or_default();
        let error = serde_json::from_str::<Value>(stderr.trim())
            .ok()
            .map(|value| value.get("error").cloned().unwrap_or(value));
        return match error {
            Some(error) => format!(
                "exit {exit}: {} {} {}",
                error["kind"], error["field"], error["reason"]
            ),
            None => format!("exit {exit}: text"),
        };
    }
    if answer["isError"] != json!(true) {
        return "ok".into();
    }
    let error = &answer["structuredContent"];
    let error = error.get("error").unwrap_or(error);
    format!(
        "mcp error: {} {} {}",
        error["kind"], error["field"], error["reason"]
    )
}

fn record(label: &str, answer: &Value, log: &mut Vec<(String, String)>) {
    eprintln!("AJN2 {}", json!({"call": label, "answer": answer}));
    log.push((label.to_owned(), outcome(answer)));
}

/// One spawned `provenance mcp` process and its connected client.
struct Server {
    child: tokio::process::Child,
    client: Client,
}

impl Server {
    async fn start(repo: &str) -> Self {
        let mut child = tokio::process::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
            .args(["mcp", "--repo", repo])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let client = Client::connect(tokio::io::join(stdout, stdin))
            .await
            .unwrap();
        Self { child, client }
    }

    async fn close(mut self) {
        self.client.cancel().await.unwrap();
        let status = self.child.wait().await.unwrap();
        assert!(status.success(), "{status}");
    }
}

fn with_actor(arguments: &Value, actor: bool) -> Value {
    let mut arguments = arguments.clone();
    if actor {
        arguments["actor"] = json!("agent");
    }
    arguments
}

fn cli_args<'a>(base: &[&'a str], repo: &'a str, actor: bool) -> Vec<&'a str> {
    let mut args = base.to_vec();
    if actor {
        args.extend(["--actor", "agent"]);
    }
    args.extend(["--repo", repo, "--format", "json"]);
    args
}

const STATEMENT: &str = "The system stores records.";

fn fixture(repo: &str) {
    ok(&["init", "--scope", "default", "--path-prefix", ".", "--path", repo]);
    ok(&["req_a", "create", "--type", "requirement", "--statement", STATEMENT, "--repo", repo]);
    ok(&[
        "rule_a", "create", "--type", "rule", "--statement", STATEMENT, "--requirement-id",
        "req_a", "--repo", repo,
    ]);
    ok(&[
        "res_a", "create", "--type", "resolution", "--title", "One store", "--position",
        "Use one store.", "--rationale", "One store is simple.", "--requirement-id", "req_a",
        "--repo", repo,
    ]);
}

/// The update of one text field, on MCP and on the CLI, with and without an actor.
async fn updates(server: &Server, repo: &str, log: &mut Vec<(String, String)>) {
    for (id, field) in [
        ("req_a", "description"),
        ("rule_a", "description"),
        ("res_a", "rationale"),
    ] {
        for actor in [false, true] {
            let text = format!("Text of the MCP update, actor {actor}.");
            let arguments = json!({"action": "update", "target": id, field: text,
                "if_match": etag(repo, id)});
            let answer = server
                .client
                .call("change", with_actor(&arguments, actor))
                .await;
            record(&format!("mcp update {id} actor={actor}"), &answer, log);
            eprintln!("AJN2 {}", json!({"decision": id, "after": decision(repo, id)}));
        }
        for actor in [false, true] {
            let flag = format!("--{field}");
            let text = format!("Text of the CLI update, actor {actor}.");
            let tag = etag(repo, id);
            let base = [id, "update", flag.as_str(), &text, "--if-match", &tag];
            let answer = run(&cli_args(&base, repo, actor));
            record(&format!("cli update {id} actor={actor}"), &answer, log);
            eprintln!("AJN2 {}", json!({"decision": id, "after": decision(repo, id)}));
        }
    }
}

/// The create of each kind, on MCP and on the CLI, with and without an actor.
async fn creates(server: &Server, repo: &str, log: &mut Vec<(String, String)>) {
    let kinds = [
        ("requirement", json!({"statement": STATEMENT, "status": "active",
            "depends_on": [], "supersedes": []})),
        ("rule", json!({"statement": STATEMENT, "requirement_ids": ["req_a"],
            "resolution_ids": [], "status": "draft", "severity": "high"})),
        ("resolution", json!({"title": "Two stores", "position": "Use two stores.",
            "rationale": "Two stores split the load.", "requirement_ids": ["req_a"],
            "supersedes": [], "inputs": [], "status": "proposed"})),
    ];
    for (kind, fields) in &kinds {
        for actor in [false, true] {
            let id = format!("{}_mcp_{actor}", &kind[..3]);
            let mut arguments = fields.clone();
            arguments["action"] = json!("create");
            arguments["target"] = json!(id);
            arguments["type"] = json!(kind);
            let answer = server
                .client
                .call("change", with_actor(&arguments, actor))
                .await;
            record(&format!("mcp create {kind} actor={actor}"), &answer, log);
        }
    }
    let cli_kinds: [(&str, Vec<&str>); 3] = [
        ("requirement", vec!["--statement", STATEMENT]),
        ("rule", vec!["--statement", STATEMENT, "--requirement-id", "req_a"]),
        ("resolution", vec!["--title", "Three stores", "--position", "Use three stores.",
            "--rationale", "Three stores split the load.", "--requirement-id", "req_a"]),
    ];
    for (kind, flags) in &cli_kinds {
        for actor in [false, true] {
            let id = format!("{}_cli_{actor}", &kind[..3]);
            let mut base = vec![id.as_str(), "create", "--type", kind];
            base.extend(flags.iter().copied());
            let answer = run(&cli_args(&base, repo, actor));
            record(&format!("cli create {kind} actor={actor}"), &answer, log);
        }
    }
}

/// Two other writes of the change tool on `req_a`: discuss and submit.
async fn others(server: &Server, repo: &str, log: &mut Vec<(String, String)>) {
    let discuss = json!({"action": "discuss", "target": "req_a", "body": "Is one store enough?",
        "role": "user"});
    let submit = json!({"action": "submit", "target": "req_a", "title": "Review req_a",
        "summary": "One store.", "source_ids": [], "evidence_references": [], "builds_on": []});
    for (action, arguments) in [("discuss", &discuss), ("submit", &submit)] {
        for actor in [false, true] {
            let answer = server
                .client
                .call("change", with_actor(arguments, actor))
                .await;
            record(&format!("mcp {action} requirement actor={actor}"), &answer, log);
        }
    }
    let cli_discuss = vec!["req_a", "discuss", "--body", "Is one store enough?"];
    let cli_submit = vec![
        "req_a", "submit", "--title", "Review req_a", "--summary", "One store.",
        "--source-ids-json", "[]", "--evidence-references-json", "[]", "--builds-on-json", "[]",
    ];
    for (action, base) in [("discuss", &cli_discuss), ("submit", &cli_submit)] {
        for actor in [false, true] {
            let answer = run(&cli_args(base, repo, actor));
            record(&format!("cli {action} requirement actor={actor}"), &answer, log);
        }
    }
}

#[tokio::test]
async fn each_write_with_and_without_an_actor_on_mcp_and_the_cli() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap().to_owned();
    fixture(&repo);
    let server = Server::start(&repo).await;
    let mut log = Vec::new();
    updates(&server, &repo, &mut log).await;
    creates(&server, &repo, &mut log).await;
    others(&server, &repo, &mut log).await;
    server.close().await;
    for (call, outcome) in &log {
        eprintln!("AJN2-OUTCOME {call} => {outcome}");
    }
    assert_eq!(log.len(), 32);
}
