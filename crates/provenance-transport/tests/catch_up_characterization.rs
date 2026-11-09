//! Characterization: the read answers after each write and hand edit,
//! and the refusals after a hand edit, through the HTTP routes. Each read
//! after a change runs the projection catch-up.
//!
//! Each answer must equal the answer of a full rebuild of the same canonical
//! state, value for value. The test also pins the whole transcript apart
//! from the random record ids. So a change to the catch-up cannot change an
//! answer, its order or a refusal.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod catch_up_steps;
    pub mod records;
    pub mod resource_http;
    pub mod transcript;
}

use provenance_transport::StatementHost;
use serde_json::{json, Value};
use support::catch_up_steps::{write, Answer, Surface, STEPS};
use support::records::{allow_reviewer, Repository};
use support::resource_http::{call, call_with_headers, host};
use support::transcript::{
    assert_same, copy_without_projection, fingerprint, named_answers, read_all, without_generated,
    Names,
};

/// The HTTP routes of one host.
struct Http(StatementHost);

impl Surface for Http {
    fn call<'a>(
        &'a self,
        method: &'a str,
        path: &'a str,
        data: Option<Value>,
        headers: &'a [(&'a str, &'a str)],
    ) -> Answer<'a> {
        let body = data.map(|data| json!({ "data": data }));
        Box::pin(call_with_headers(&self.0, method, path, body, headers))
    }
}

fn state(repo: &Repository) -> std::path::PathBuf {
    repo.layout.state_dir().into_std_path_buf()
}

/// The answers of a full rebuild of the canonical state of `repo`.
async fn rebuilt_answers(repo: &Repository) -> Vec<Value> {
    let copy = copy_without_projection(repo);
    read_all(&Http(host(&copy, false)), &copy).await
}

/// Implementation aid: characterization of the read answers after each
/// change, made before the catch-up change. No Rule binds it.
#[tokio::test]
async fn reads_after_each_write_equal_a_full_rebuild() {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let http = Http(host(&repo, true));
    let mut transcript = Vec::new();
    for step in 0..STEPS {
        write(&http, &state(&repo), step).await;
        let caught_up = read_all(&http, &repo).await;
        assert_same(
            &caught_up,
            &rebuilt_answers(&repo).await,
            &format!("step {step}"),
        );
        let answers = named_answers(&mut Names::default(), &caught_up);
        transcript.push(json!({"step": step, "answers": answers}));
    }
    let text = serde_json::to_string_pretty(&transcript).unwrap();
    assert_eq!(fingerprint(&transcript[..15]), "841ce06d32c8aee2", "{text}");
    assert_eq!(fingerprint(&transcript), "0786f7337c0b8f94", "{text}");
}

/// A hand edit that each validator refuses, and the answer the next read
/// gives at the stored serial.
async fn refusal(edit: impl FnOnce(&Repository)) -> Value {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let host = host(&repo, true);
    for step in 0..8 {
        write(&Http(host.clone()), &state(&repo), step).await;
    }
    call(&host, "GET", "/requirements", None).await;
    edit(&repo);
    let root = repo.dir.path().to_str().unwrap().to_owned();
    let mut answers = Vec::new();
    for path in ["/requirements", "/topics/topic_a", "/proposals"] {
        let (status, body) = call(&host, "GET", path, None).await;
        let error = body["meta"]["freshness_error"]
            .as_str()
            .map(|text| without_generated(&text.replace(&root, "<repo>")));
        let items = body["data"]["items"].as_array().map(Vec::len);
        answers.push(
            json!({"read": path, "status": status, "freshness_error": error,
            "items": items, "error": body["error"]}),
        );
    }
    json!(answers)
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
    let card_error = "duplicate immutable proposal id <generated>";
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
