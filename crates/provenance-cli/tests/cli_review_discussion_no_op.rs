//! A Discussion status write that changes nothing sends no change event.
#[path = "review_host_support/mod.rs"]
#[allow(dead_code)]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{
    events::{source, Socket},
    repository, request, response, start, Host,
};
use serde_json::{json, Value};

const PARENT: &str = "/sources/source_talk/discussions";

fn send(host: &Host, method: &str, path: &str, guard: Option<&str>, body: Value) -> Value {
    let mut call = request(host, method, path, true).set("Content-Type", "application/json");
    if let Some(guard) = guard {
        call = call.set("If-Match", guard);
    }
    let answer = response(call.send_string(&json!({ "data": body }).to_string()));
    assert_eq!(answer.status(), 200, "{method} {path}");
    serde_json::from_str(&answer.into_string().unwrap()).unwrap()
}

/// Reads frames until one names the source `id`.
fn frames_until(socket: &mut Socket, id: &str) {
    while !socket.frame().to_string().contains(id) {}
}

#[test]
#[verifies("rule_discussion_same_status_is_no_op", examples)]
fn a_repeated_discussion_status_sends_no_change_event() {
    let repo = repository();
    let host = start(repo.path());
    let mut socket = Socket::connect(&host);
    assert_eq!(socket.frame()["type"], "ready");
    source(&host, repo.path(), "source_talk");
    let started = send(
        &host,
        "POST",
        PARENT,
        None,
        json!({"actor":"reviewer", "declared_by":null, "role":"user", "body":"A concern."}),
    );
    let discussion = format!(
        "{PARENT}/{}",
        started["data"]["discussion_id"].as_str().unwrap()
    );
    let status = json!({"actor":"reviewer", "declared_by":null, "status":"resolved"});
    send(&host, "PATCH", &discussion, Some("\"1\""), status.clone());
    // The marker write comes after the earlier writes, so its frame is the last of them.
    source(&host, repo.path(), "source_marker");
    frames_until(&mut socket, "source_marker");

    let repeated = send(&host, "PATCH", &discussion, Some("\"2\""), status);
    assert_eq!(repeated["data"]["version"], 2);
    source(&host, repo.path(), "source_after");

    let next = socket.frame();
    assert_eq!(next["type"], "changed", "{next}");
    assert_eq!(
        next["scopes"],
        json!([{"scope":"default", "records":[{"kind":"source", "id":"source_after"}], "families":["source"]}])
    );
    socket.quiet();
}
