//! A Requirement create, a Requirement update and a Discussion start through the HTTP API
//! that give no actor record the actor `authoring`. A reply needs no actor. A Discussion
//! status change needs its actor, and a JSON `null` actor is refused on each write.
#![cfg(feature = "test-fixture")]
use crate::support;
use provenance_macros::verifies;
use serde_json::{json, Value};
use support::records::Repository;
use support::resource_http::{call, call_with_headers, host};

const PARENT: &str = "/requirements/req_shared/discussions";

/// The stored state of the scope `default` of the fixture repository.
fn state(repo: &Repository) -> std::path::PathBuf {
    repo.dir.path().join(".provenance/state/scopes/default")
}

/// The stored rows of the file `name` below the stored state of the scope `default`.
fn rows(repo: &Repository, name: &str) -> Vec<Value> {
    let text = std::fs::read_to_string(state(repo).join(name)).unwrap();
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// The stored Discussion rows of the fixture repository.
fn discussion_rows(repo: &Repository) -> Vec<Value> {
    rows(repo, "threads/discussions.jsonl")
}

/// Starts a Discussion of `req_shared` with the actor `reviewer` and gives its path.
async fn discussion(host: &provenance_transport::StatementHost) -> String {
    let body = json!({"data":{"actor":"reviewer", "declared_by":null, "role":"user",
        "body":"A concern."}});
    let (status, started) = call(host, "POST", PARENT, Some(body)).await;
    assert_eq!(status, 200, "{started}");
    format!(
        "{PARENT}/{}",
        started["data"]["discussion_id"].as_str().unwrap()
    )
}

/// The data of one successful read.
async fn read(host: &provenance_transport::StatementHost, path: &str) -> Value {
    let (status, read) = call(host, "GET", path, None).await;
    assert_eq!(status, 200, "{read}");
    read["data"].clone()
}

#[tokio::test]
#[verifies("rule_write_without_actor_records_authoring", examples)]
async fn an_api_requirement_create_with_no_actor_records_authoring() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let body = json!({"data":{"id":"req_new", "statement":"The system stores each record.",
        "status":"active", "depends_on":[], "supersedes":[]}});
    let (status, created) = call(&host, "POST", "/requirements", Some(body)).await;
    assert_eq!(status, 200, "{created}");
    let stored = read(&host, "/requirements/req_new").await;
    assert_eq!(stored["decision"]["pending"]["actor"], "authoring");
    assert_eq!(
        (&stored["id"], &stored["statement"], &stored["status"]),
        (
            &json!("req_new"),
            &json!("The system stores each record."),
            &json!("active")
        )
    );
    assert_eq!(created, json!({"data": stored, "meta": {}}));
}

#[tokio::test]
#[verifies("rule_write_without_actor_records_authoring", examples)]
async fn an_api_requirement_update_with_no_actor_records_authoring() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let etag = read(&host, "/requirements/req_shared").await["edit"]["etag"]
        .as_str()
        .unwrap()
        .to_owned();
    let body = json!({"data":{"description":"Edited."}});
    let (status, updated) = call_with_headers(
        &host,
        "PATCH",
        "/requirements/req_shared",
        Some(body),
        &[("if-match", &etag)],
    )
    .await;
    assert_eq!(status, 200, "{updated}");
    let stored = read(&host, "/requirements/req_shared").await;
    assert_eq!(stored["decision"]["pending"]["actor"], "authoring");
    assert_eq!(stored["description"], "Edited.");
    assert_eq!(updated, json!({"data": stored, "meta": {}}));
}

#[tokio::test]
#[verifies("rule_write_without_actor_records_authoring", examples)]
async fn an_api_discussion_start_with_no_actor_records_authoring() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let body = json!({"data":{"declared_by":null, "role":"user", "body":"A concern."}});
    let (status, started) = call(&host, "POST", PARENT, Some(body)).await;
    assert_eq!(status, 200, "{started}");
    let id = started["data"]["discussion_id"].as_str().unwrap();
    let stored = read(&host, &format!("{PARENT}/{id}")).await["discussion"].clone();
    let root = stored["root_message_id"].clone();
    assert_eq!(
        stored,
        json!({
            "scope_id": "default", "discussion_id": id,
            "parent": {"node_type": "requirement", "node_id": "req_shared"},
            "thread_id": "thread_requirement_req_shared", "root_message_id": root,
            "message_ids": [root], "status": "active", "version": 1, "actor": "authoring",
            "outcomes": [], "disposition_id": null,
        })
    );
    assert_eq!(started, json!({"data": stored, "meta": {}}));
}

/// A status change names the actor who makes it, so a status change with no actor is
/// refused and the Discussion stays as it is.
#[tokio::test]
#[verifies("rule_write_actor_required", examples)]
async fn an_api_discussion_status_change_with_no_actor_is_refused() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let discussion = discussion(&host).await;
    let body = json!({"data":{"declared_by":null, "status":"resolved"}});
    let (status, refused) = call_with_headers(
        &host,
        "PATCH",
        &discussion,
        Some(body),
        &[("if-match", "\"1\"")],
    )
    .await;
    assert_eq!(status, 400, "{refused}");
    assert_eq!(
        refused,
        json!({"error":{"kind":"invalid_input", "field":"request.actor", "reason":"required"},
            "meta":{}})
    );
    let stored = read(&host, &discussion).await["discussion"].clone();
    assert_eq!(
        (&stored["status"], &stored["version"]),
        (&json!("active"), &json!(1))
    );
}

/// A status change with no actor is refused on its actor before the host opens the
/// repository, so the refusal does not change when the repository has a fault too. The same
/// status change with an actor gets the fault of the repository.
#[tokio::test]
#[verifies("rule_write_actor_required", examples)]
async fn a_status_change_with_no_actor_is_refused_before_the_host_opens_the_repository() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let discussion = discussion(&host).await;
    std::fs::remove_file(repo.layout.manifest_path()).unwrap();
    let mut answers = Vec::new();
    for actor in [None, Some("reviewer")] {
        let mut data = json!({"declared_by":null, "status":"resolved"});
        if let Some(actor) = actor {
            data["actor"] = json!(actor);
        }
        let answer = call_with_headers(
            &host,
            "PATCH",
            &discussion,
            Some(json!({"data": data})),
            &[("if-match", "\"1\"")],
        )
        .await;
        answers.push(answer);
    }
    assert_eq!(
        answers,
        [
            (
                400,
                json!({"error":{"kind":"invalid_input", "field":"request.actor",
                    "reason":"required"}, "meta":{}})
            ),
            (500, json!({"error":{"kind":"internal"}, "meta":{}})),
        ]
    );
}

/// Implementation aid: no Rule states what a reply stores for its actor. A reply with an
/// actor and a reply with no actor are accepted. The message rows have no actor, and the
/// Discussion keeps the actor of its start.
#[tokio::test]
async fn an_api_reply_with_or_without_an_actor_stores_no_actor() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let discussion = discussion(&host).await;
    let replies = format!("{discussion}/messages");
    let mut answers = Vec::new();
    for (version, actor) in [(1, Some("agent")), (2, None)] {
        let mut data = json!({"declared_by":null, "role":"user", "body":"A reply."});
        if let Some(actor) = actor {
            data["actor"] = json!(actor);
        }
        let header = format!("\"{version}\"");
        let (status, answer) = call_with_headers(
            &host,
            "POST",
            &replies,
            Some(json!({"data": data})),
            &[("if-match", &header)],
        )
        .await;
        assert_eq!(status, 200, "{actor:?}: {answer}");
        answers.push(answer);
    }
    let discussions = discussion_rows(&repo);
    assert_eq!(discussions.len(), 1);
    assert_eq!(
        (&discussions[0]["actor"], &discussions[0]["version"]),
        (&json!("reviewer"), &json!(3))
    );
    let threads = state(&repo).join("threads");
    let mut messages = Vec::new();
    for entry in std::fs::read_dir(&threads).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        if name != "discussions.jsonl" && name != "threads.jsonl" {
            messages.extend(rows(&repo, &format!("threads/{name}")));
        }
    }
    let message_ids = messages
        .iter()
        .map(|row| row["id"].clone())
        .collect::<Vec<_>>();
    assert_eq!(discussions[0]["message_ids"], json!(message_ids));
    let thread = &discussions[0]["thread_id"];
    for (row, body) in messages.iter().zip(["A concern.", "A reply.", "A reply."]) {
        let expected = json!({
            "scope_id": "default", "id": row["id"], "thread_id": thread, "role": "user",
            "body": body, "created_at": row["created_at"],
        });
        assert_eq!(row, &expected);
    }
    let id = discussion.rsplit('/').next().unwrap();
    let receipt = |version: usize| {
        json!({
            "scope_id": "default", "discussion_id": id,
            "parent": {"node_type": "requirement", "node_id": "req_shared"},
            "thread_id": thread, "root_message_id": message_ids[0],
            "message_ids": message_ids[..version], "status": "active", "version": version,
            "actor": "reviewer", "outcomes": [], "disposition_id": null,
        })
    };
    let expected = [2, 3].map(|version| json!({"data": receipt(version), "meta": {}}));
    assert_eq!(answers, expected);
}

/// Implementation aid: a JSON `null` actor is invalid input on each write that takes an
/// optional actor, and the write changes no record.
#[tokio::test]
async fn a_null_actor_is_refused_on_each_write() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let discussion = discussion(&host).await;
    let before = read(&host, "/requirements/req_shared").await;
    let etag = before["edit"]["etag"].as_str().unwrap().to_owned();
    let message = json!({"actor":null, "declared_by":null, "role":"user", "body":"A reply."});
    let writes = [
        (
            "POST",
            "/requirements".to_owned(),
            json!({"actor":null, "id":"req_new", "statement":"The system stores each record.",
                "status":"active", "depends_on":[], "supersedes":[]}),
            None,
        ),
        (
            "PATCH",
            "/requirements/req_shared".to_owned(),
            json!({"actor":null, "description":"Edited."}),
            Some(etag.as_str()),
        ),
        (
            "POST",
            PARENT.to_owned(),
            json!({"actor":null, "declared_by":null, "role":"user", "body":"A concern."}),
            None,
        ),
        (
            "POST",
            format!("{discussion}/messages"),
            message,
            Some("\"1\""),
        ),
    ];
    let mut answers = Vec::new();
    for (method, path, data, if_match) in writes {
        let headers = if_match.map_or_default(|value| vec![("if-match", value)]);
        let answer =
            call_with_headers(&host, method, &path, Some(json!({"data": data})), &headers).await;
        answers.push(answer);
    }
    let refused = |field: Value| {
        (
            400,
            json!({"error":{"kind":"invalid_input", "field":field, "reason":"invalid_value"},
                "meta":{}}),
        )
    };
    assert_eq!(
        answers,
        [
            refused(json!("request.actor")),
            refused(Value::Null),
            refused(json!("request.actor")),
            refused(json!("request.actor")),
        ]
    );
    let (status, _) = call(&host, "GET", "/requirements/req_new", None).await;
    assert_eq!(status, 404);
    assert_eq!(read(&host, "/requirements/req_shared").await, before);
    let discussions = discussion_rows(&repo);
    assert_eq!(discussions.len(), 1);
    assert_eq!(discussions[0]["version"], 1);
}
