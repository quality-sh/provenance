//! Characterization: the read answers after each write, and the refusals
//! after a hand edit, through the HTTP routes. Each read after a write runs
//! the projection catch-up. The test pins that the answers equal the
//! answers of a full rebuild of the same canonical state, step by step,
//! and pins the whole transcript apart from the random record ids, so a
//! change to how catch-up derives the projection cannot change an answer,
//! its order or a refusal.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
    pub mod resource_http;
    pub mod transcript;
}

use provenance_transport::StatementHost;
use serde_json::{json, Value};
use support::records::{allow_reviewer, Repository};
use support::resource_http::{call, call_with_headers, host};
use support::transcript::{fingerprint, read_all, rebuilt_answers, Names};

async fn ok(host: &StatementHost, method: &str, path: &str, data: Value) -> Value {
    let (status, value) = call(host, method, path, Some(json!({ "data": data }))).await;
    assert_eq!(status, 200, "{method} {path}: {value}");
    value["data"].clone()
}

async fn patch(host: &StatementHost, path: &str, data: Value) {
    let (status, before) = call(host, "GET", path, None).await;
    assert_eq!(status, 200, "{path}: {before}");
    let etag = before["data"]["edit"]["etag"].as_str().unwrap().to_owned();
    let (status, saved) = call_with_headers(
        host,
        "PATCH",
        path,
        Some(json!({ "data": data })),
        &[("If-Match", etag.as_str())],
    )
    .await;
    assert_eq!(status, 200, "PATCH {path}: {saved}");
}

async fn pending(host: &StatementHost, path: &str) -> String {
    let (status, record) = call(host, "GET", path, None).await;
    assert_eq!(status, 200, "{path}: {record}");
    record["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap_or_else(|| panic!("{path} has no pending submission: {record}"))
        .to_owned()
}

/// One write of the fixture sequence. Each step changes a different set of
/// shards: a new record and its review submission, a relation that comes
/// and goes, a title only, a decision, a withdrawal and a discussion.
async fn write(host: &StatementHost, step: usize) {
    if step <= 7 {
        create(host, step).await;
    } else {
        change(host, step).await;
    }
}

/// The steps that create one record each.
async fn create(host: &StatementHost, step: usize) {
    match step {
        0 => {}
        1 => {
            ok(
                host,
                "POST",
                "/requirements",
                json!({"actor":"agent", "id":"req_a",
                "statement":"The record req_a is readable.", "status":"discovery",
                "refines":["req_shared"], "depends_on":[], "supersedes":[]}),
            )
            .await;
        }
        2 => {
            ok(
                host,
                "POST",
                "/requirements",
                json!({"actor":"agent", "id":"req_b",
                "statement":"The record req_b is readable.", "status":"discovery",
                "refines":["req_shared"], "depends_on":["req_a"], "supersedes":[]}),
            )
            .await;
        }
        3 => {
            ok(
                host,
                "POST",
                "/sources",
                json!({"id":"source_a", "name":"Source A",
                "source_type":"document", "url":null, "reference":"docs/a.md",
                "commit_pin":null, "supersedes":[], "origin_thread":null,
                "origin_message":null}),
            )
            .await;
        }
        4 => {
            ok(
                host,
                "POST",
                "/resolutions",
                json!({"id":"res_a", "title":"Decision A",
                "position":"Use the record.", "rationale":"The record is available.",
                "status":"draft", "requirement_ids":["req_a"], "supersedes":[],
                "inputs":[]}),
            )
            .await;
        }
        5 => {
            ok(
                host,
                "POST",
                "/rules",
                json!({"id":"rule_a",
                "statement":"The host keeps each record.", "status":"active",
                "severity":"high", "requirement_ids":["req_a"], "resolution_ids":[]}),
            )
            .await;
        }
        6 => {
            ok(
                host,
                "POST",
                "/topics",
                json!({"id":"topic_a", "requirement_id":"req_a",
                "title":"Topic A", "status":"open",
                "links":[{"target_type":"source","target_id":"source_a"}]}),
            )
            .await;
        }
        7 => {
            ok(
                host,
                "POST",
                "/questions",
                json!({"id":"q_a", "topic_id":"topic_a",
                "question":"Which record is first?", "resolution_method":"research",
                "status":"open", "links":[{"target_type":"rule","target_id":"rule_a"}]}),
            )
            .await;
        }
        _ => unreachable!("no create step {step}"),
    }
}

/// The steps that change or decide a record, or start a discussion.
async fn change(host: &StatementHost, step: usize) {
    match step {
        8 => patch(host, "/topics/topic_a", json!({"title":"Topic A, renamed"})).await,
        9 => {
            patch(
                host,
                "/requirements/req_b",
                json!({"actor":"agent", "relationships":{"depends_on":{"remove":["req_a"]}}}),
            )
            .await;
        }
        10 => {
            patch(
                host,
                "/requirements/req_b",
                json!({"actor":"agent", "statement":"The record req_b is new."}),
            )
            .await;
        }
        11 => {
            let proposal = pending(host, "/requirements/req_a").await;
            ok(
                host,
                "POST",
                &format!("/requirements/req_a/submissions/{proposal}/decide"),
                json!({"actor":{"identity_type":"human","id":"reviewer"},
                    "decision":"accepted", "rationale":null,
                    "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_a"},
                    "feedback":null, "declared_by":null}),
            )
            .await;
        }
        12 => {
            let proposal = pending(host, "/requirements/req_b").await;
            ok(
                host,
                "POST",
                &format!("/requirements/req_b/submissions/{proposal}/withdraw"),
                json!({"actor":"agent", "declared_by":null, "reason":"Revise it."}),
            )
            .await;
        }
        13 => patch(host, "/questions/q_a", json!({"links":[]})).await,
        14 => {
            ok(
                host,
                "POST",
                "/requirements/req_a/discussions",
                json!({"actor":"reviewer",
                "declared_by":null, "role":"user", "body":"Name the scope."}),
            )
            .await;
        }
        _ => unreachable!("no change step {step}"),
    }
}

const STEPS: usize = 15;

/// Implementation aid: characterization of the read answers after each
/// write before the catch-up change. No Rule binds it.
#[tokio::test]
async fn reads_after_each_write_equal_a_full_rebuild() {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let host = host(&repo, true);
    let mut transcript = Vec::new();
    for step in 0..STEPS {
        write(&host, step).await;
        let caught_up = read_all(&host, &repo, &mut Names::default()).await;
        let rebuilt = rebuilt_answers(&repo).await;
        for (index, (left, right)) in caught_up.iter().zip(&rebuilt).enumerate() {
            assert_eq!(left, right, "step {step}, read {index}");
        }
        transcript.push(json!({"step": step, "answers": caught_up}));
    }
    let text = serde_json::to_string_pretty(&transcript).unwrap();
    assert_eq!(fingerprint(&transcript), "89229c919cd763ba", "{text}");
}

/// A hand edit that each validator refuses, and the answer the next read
/// gives at the stored serial.
async fn refusal(edit: impl FnOnce(&Repository)) -> Value {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let host = host(&repo, true);
    for step in 0..8 {
        write(&host, step).await;
    }
    call(&host, "GET", "/requirements", None).await;
    edit(&repo);
    let root = repo.dir.path().to_str().unwrap().to_owned();
    let mut answers = Vec::new();
    for path in ["/requirements", "/topics/topic_a", "/proposals"] {
        let (status, body) = call(&host, "GET", path, None).await;
        let error = body["meta"]["freshness_error"]
            .as_str()
            .map(|text| without_uuids(&text.replace(&root, "<repo>")));
        let items = body["data"]["items"].as_array().map(Vec::len);
        answers.push(
            json!({"read": path, "status": status, "freshness_error": error,
            "items": items, "error": body["error"]}),
        );
    }
    json!(answers)
}

/// The text with each generated UUID replaced, since card ids are random.
fn without_uuids(text: &str) -> String {
    text.split(' ')
        .map(|word| {
            let uuid = word.len() == 36 && word.split('-').map(str::len).eq([8, 4, 4, 4, 12]);
            if uuid {
                "<uuid>"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn append(repo: &Repository, shard: &str, line: &str) {
    let path = repo.layout.state_dir().join("scopes/default").join(shard);
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(line);
    text.push('\n');
    std::fs::write(path, text).unwrap();
}

/// Implementation aid: characterization of the refusals that a hand edit
/// gives before the catch-up change. No Rule binds it.
#[tokio::test]
async fn hand_edits_that_validation_refuses_keep_their_refusals() {
    let cycle = refusal(|repo| {
        append(
            repo,
            "requirements/req.jsonl",
            &json!({"scope_id":"default", "id":"req_c",
            "statement":"The record req_c is readable.", "status":"discovery",
            "depends_on":["req_d"], "supersedes":[]})
            .to_string(),
        );
        append(
            repo,
            "requirements/req.jsonl",
            &json!({"scope_id":"default", "id":"req_d",
            "statement":"The record req_d is readable.", "status":"discovery",
            "depends_on":["req_c"], "supersedes":[]})
            .to_string(),
        );
    })
    .await;
    let duplicate_card = refusal(|repo| {
        let path = repo
            .layout
            .state_dir()
            .join("scopes/default/ideation/proposal_cards.jsonl");
        let first = std::fs::read_to_string(&path)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_owned();
        append(repo, "ideation/proposal_cards.jsonl", &first);
    })
    .await;
    let broken_topic = refusal(|repo| append(repo, "topics/topic.jsonl", "{\"id\":")).await;
    let missing_artifact = refusal(|repo| {
        let path = repo
            .layout
            .state_dir()
            .join("scopes/default/topics/topic.jsonl");
        std::fs::write(path, "").unwrap();
    })
    .await;
    let at_stored = |error: &str, topic: Value| {
        json!([
            {"read": "/requirements", "status": 200, "freshness_error": error, "items": 3,
                "error": null},
            topic,
            {"read": "/proposals", "status": 200, "freshness_error": error, "items": 9,
                "error": null},
        ])
    };
    let topic = |error: &str| {
        json!({"read": "/topics/topic_a", "status": 200, "freshness_error": error,
            "items": null, "error": null})
    };
    let cycle_error = "depends_on forms a cycle: req_c -> req_d -> req_c";
    assert_eq!(cycle, at_stored(cycle_error, topic(cycle_error)));
    let card_error = "duplicate immutable proposal id <uuid>";
    assert_eq!(duplicate_card, at_stored(card_error, topic(card_error)));
    let line_error = "scopes/default/topics/topic.jsonl line 2: EOF while parsing a value";
    let unreadable = json!({"read": "/topics/topic_a", "status": 500, "freshness_error": null,
        "items": null, "error": {"file": "scopes/default/topics/topic.jsonl",
            "kind": "invalid_state_file", "line": 2, "message": "EOF while parsing a value"}});
    assert_eq!(broken_topic, at_stored(line_error, unreadable));
    assert_eq!(
        missing_artifact,
        json!([
            {"read": "/requirements", "status": 200, "freshness_error": null, "items": 3,
                "error": null},
            {"read": "/topics/topic_a", "status": 404, "freshness_error": null, "items": null,
                "error": {"kind": "resource_not_found"}},
            {"read": "/proposals", "status": 200, "freshness_error": null, "items": 9,
                "error": null},
        ])
    );
}
