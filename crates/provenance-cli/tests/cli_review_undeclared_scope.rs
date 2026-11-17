//! A scope that the manifest stops declaring after the review host and the MCP
//! server start: a read and a write through the MCP `api` tool and through the
//! HTTP API of the review host each refuse with `unknown_scope`.
//!
//! Implementation aid with no Rule binding: no Rule states the refusal of a
//! scope that the manifest stops declaring after a host opens the repository.
use crate::review_host_support;
use review_host_support::{follow::create, repository_with_actors, request, start};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Stdio;

/// Rewrites the manifest so that it declares only the scope `other`.
fn declare_only_another_scope(root: &Path) {
    let path = root.join(".provenance/state/manifest.json");
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    manifest["scopes"] = json!([{"id": "other", "path_prefix": "."}]);
    review_host_support::replace_file(&path, format!("{manifest}\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_scope_that_the_manifest_stops_declaring_refuses_each_read_and_write() {
    let repo = repository_with_actors(&["reviewer"]);
    create(repo.path(), "req_scope", "requirement", &[]);
    let host = start(repo.path());
    let mut child = tokio::process::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(["mcp", "--repo", repo.path().to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let io = tokio::io::join(child.stdout.take().unwrap(), child.stdin.take().unwrap());
    let client = provenance_mcp::Client::connect(io).await.unwrap();
    declare_only_another_scope(repo.path());

    let created = json!({"actor":"reviewer", "id":"req_new",
        "statement":"The system stores each record.", "status":"discovery", "depends_on":[],
        "supersedes":[]});
    let mut answers = Vec::new();
    for arguments in [
        json!({"path":"requirements/req_scope", "method":"get"}),
        json!({"path":"requirements", "method":"post", "input":created}),
    ] {
        let method = arguments["method"].clone();
        let answer = client.call("api", arguments).await;
        answers.push(json!({"mcp": method, "is_error": answer["isError"],
            "error": answer["structuredContent"]["error"]}));
    }
    client.cancel().await.unwrap();
    child.wait().await.unwrap();
    let body = json!({"data": created}).to_string();
    let http = tokio::task::spawn_blocking(move || {
        [
            ("GET", "/requirements/req_scope", None),
            ("POST", "/requirements", Some(body)),
        ]
        .map(|(method, route, body)| {
            let call = request(&host, method, route, true).set("Content-Type", "application/json");
            let reply = match body {
                Some(body) => call.send_string(&body),
                None => call.call(),
            };
            let response = match reply {
                Ok(response) | Err(ureq::Error::Status(_, response)) => response,
                Err(error) => panic!("{method} {route}: {error}"),
            };
            let status = response.status();
            let answer: Value = serde_json::from_str(&response.into_string().unwrap()).unwrap();
            json!({"http": format!("{method} {route}"), "status": status,
                "error": answer["error"]})
        })
    })
    .await
    .unwrap();
    answers.extend(http);

    let refusal = json!({"kind": "unknown_scope"});
    assert_eq!(
        answers,
        [
            json!({"mcp": "get", "is_error": true, "error": refusal}),
            json!({"mcp": "post", "is_error": true, "error": refusal}),
            json!({"http": "GET /requirements/req_scope", "status": 404, "error": refusal}),
            json!({"http": "POST /requirements", "status": 404, "error": refusal}),
        ]
    );
}
