//! Characterization: the review state of each record and the review document
//! pages over one fixture with several records, submissions, decisions,
//! feedback, withdrawals and revisions. The test pins the observed answers so
//! a change to how the review state is computed cannot change them.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
    pub mod resource_http;
}

use provenance_transport::StatementHost;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use support::records::{allow_reviewer, Repository};
use support::resource_http::{call, call_with_headers, host};

/// Gives each generated id a stable name: the name the fixture recorded for
/// it, or else a sequence name in the order the test first sees it.
#[derive(Default)]
struct Names {
    known: BTreeMap<String, String>,
    unknown: usize,
}

impl Names {
    fn record(&mut self, id: &Value, name: &str) {
        let id = id.as_str().unwrap_or_else(|| panic!("{name}: {id}"));
        assert!(
            self.known.insert(id.to_owned(), name.to_owned()).is_none(),
            "{name}"
        );
    }

    fn name(&mut self, id: &Value) -> Value {
        let Some(id) = id.as_str() else {
            return id.clone();
        };
        if let Some(name) = self.known.get(id) {
            return json!(name);
        }
        self.unknown += 1;
        let name = format!("other_{}", self.unknown);
        self.known.insert(id.to_owned(), name.clone());
        json!(name)
    }
}

async fn ok(host: &StatementHost, method: &str, path: &str, data: Option<Value>) -> Value {
    let (status, value) = call(host, method, path, data.map(|data| json!({ "data": data }))).await;
    assert_eq!(status, 200, "{method} {path}: {value}");
    value["data"].clone()
}

async fn create(
    host: &StatementHost,
    names: &mut Names,
    id: &str,
    refines: &[&str],
    depends_on: &[&str],
) -> Value {
    let created = ok(
        host,
        "POST",
        "/requirements",
        Some(json!({
            "actor":"agent", "id":id, "statement":format!("The record {id} is readable."),
            "status":"discovery", "refines":refines, "depends_on":depends_on, "supersedes":[]
        })),
    )
    .await;
    names.record(&created["edit"]["revision"], &format!("{id}@1"));
    names.record(
        &created["decision"]["pending"]["proposal_id"],
        &format!("{id}#1"),
    );
    created
}

async fn change(host: &StatementHost, names: &mut Names, id: &str, text: &str, step: u8) -> Value {
    let path = format!("/requirements/{id}");
    let before = ok(host, "GET", &path, None).await;
    let (status, saved) = call_with_headers(
        host,
        "PATCH",
        &path,
        Some(json!({"data":{"actor":"agent","statement":text}})),
        &[("If-Match", before["edit"]["etag"].as_str().unwrap())],
    )
    .await;
    assert_eq!(status, 200, "{saved}");
    let saved = saved["data"].clone();
    let revision = &saved["edit"]["revision"];
    if !names.known.contains_key(revision.as_str().unwrap()) {
        names.record(revision, &format!("{id}@{step}"));
    }
    if let Some(pending) = saved["decision"]["pending"]["proposal_id"].as_str() {
        if !names.known.contains_key(pending) {
            names.record(&json!(pending), &format!("{id}#{step}"));
        }
    }
    saved
}

async fn decide(
    host: &StatementHost,
    names: &mut Names,
    path: &str,
    proposal: &str,
    decision: &str,
    feedback: bool,
) {
    let kind = if path.starts_with("/resolutions") {
        "resolution"
    } else {
        "requirement"
    };
    let id = path.rsplit('/').next().unwrap();
    let decided = ok(
        host,
        "POST",
        &format!("{path}/submissions/{proposal}/decide"),
        Some(json!({
            "actor":{"identity_type":"human","id":"reviewer"},
            "decision":decision,
            "rationale": (decision == "rejected").then_some("Name the condition."),
            "canonical_artifact": (decision == "accepted")
                .then(|| json!({"artifact_type":kind,"artifact_id":id})),
            "feedback": feedback.then(|| json!({"role":"user","body":"Name the scope."})),
            "declared_by":null
        })),
    )
    .await;
    let name = names.known[proposal].clone();
    names.record(&decided["disposition_id"], &format!("{name}:{decision}"));
}

async fn withdraw(host: &StatementHost, path: &str, proposal: &str) {
    ok(
        host,
        "POST",
        &format!("{path}/submissions/{proposal}/withdraw"),
        Some(json!({"actor":"agent","declared_by":null,"reason":"Revise it."})),
    )
    .await;
}

fn pending(names: &Names, name: &str) -> String {
    names
        .known
        .iter()
        .find_map(|(id, known)| (known == name).then(|| id.clone()))
        .unwrap_or_else(|| panic!("no id named {name}"))
}

/// The fixture. Each requirement except `req_outside` refines `req_shared`,
/// which the repository seeds with `rule_shared`. `req_waiting` cites
/// `req_outside`, so the document has one reference.
async fn create_records(host: &StatementHost, names: &mut Names) {
    create(host, names, "req_outside", &[], &[]).await;
    for id in [
        "req_accepted",
        "req_rejected",
        "req_revised",
        "req_resubmitted",
        "req_withdrawn",
        "req_returned",
    ] {
        create(host, names, id, &["req_shared"], &[]).await;
    }
    create(
        host,
        names,
        "req_waiting",
        &["req_shared"],
        &["req_outside"],
    )
    .await;
    ok(
        host,
        "POST",
        "/resolutions",
        Some(json!({
            "id":"resolution_choice", "title":"Decision", "position":"Use the record.",
            "rationale":"The record is available.", "status":"draft",
            "requirement_ids":["req_shared"], "supersedes":[], "inputs":[]
        })),
    )
    .await;
    let resolution = ok(host, "GET", "/resolutions/resolution_choice", None).await;
    names.record(
        &resolution["decision"]["pending"]["proposal_id"],
        "resolution_choice#1",
    );
    names.record(
        &resolution["decision"]["current_revision"],
        "resolution_choice@1",
    );
}

/// The decisions, revisions, resubmission and withdrawal of the fixture.
async fn review(host: &StatementHost, names: &mut Names) {
    for (path, name, decision, feedback) in [
        (
            "/resolutions/resolution_choice",
            "resolution_choice#1",
            "accepted",
            false,
        ),
        (
            "/requirements/req_accepted",
            "req_accepted#1",
            "accepted",
            true,
        ),
        (
            "/requirements/req_rejected",
            "req_rejected#1",
            "rejected",
            true,
        ),
        (
            "/requirements/req_revised",
            "req_revised#1",
            "rejected",
            false,
        ),
        (
            "/requirements/req_resubmitted",
            "req_resubmitted#1",
            "rejected",
            true,
        ),
        (
            "/requirements/req_returned",
            "req_returned#1",
            "accepted",
            false,
        ),
    ] {
        let proposal = pending(names, name);
        decide(host, names, path, &proposal, decision, feedback).await;
    }
    change(
        host,
        names,
        "req_revised",
        "The revised record is readable.",
        2,
    )
    .await;
    let rejected = pending(names, "req_resubmitted#1");
    let resubmitted = ok(
        host,
        "POST",
        "/requirements/req_resubmitted/submit",
        Some(json!({
            "actor":"agent", "declared_by":null, "title":"Keep the record",
            "summary":"The record stays readable.", "source_ids":[],
            "evidence_references":[], "builds_on":[], "revises":rejected
        })),
    )
    .await;
    names.record(&resubmitted["proposal_id"], "req_resubmitted#2");
    let proposal = pending(names, "req_withdrawn#1");
    withdraw(host, "/requirements/req_withdrawn", &proposal).await;
    change(
        host,
        names,
        "req_returned",
        "The returned record is new.",
        2,
    )
    .await;
    change(
        host,
        names,
        "req_returned",
        "The record req_returned is readable.",
        3,
    )
    .await;
}

/// The decision state of one record with each generated id named.
fn decision_state(names: &mut Names, decision: &Value) -> Value {
    let decided = |names: &mut Names, decision: &Value| {
        json!({
            "revision": names.name(&decision["revision"]),
            "proposal": names.name(&decision["disposition"]["proposal_id"]),
            "disposition": names.name(&decision["disposition"]["id"]),
            "decision": decision["disposition"]["decision"],
            "feedback": !decision["feedback_message_id"].is_null(),
        })
    };
    let pending = &decision["pending"];
    json!({
        "current_revision": names.name(&decision["current_revision"]),
        "pending": if pending.is_null() { Value::Null } else { json!({
            "proposal": names.name(&pending["proposal_id"]),
            "revision": names.name(&pending["revision"]),
            "revises": names.name(&pending["revises"]),
        })},
        "current_acceptance": if decision["current_acceptance"].is_null() {
            Value::Null
        } else {
            decided(names, &decision["current_acceptance"])
        },
        "decisions": decision["decisions"].as_array().unwrap().iter()
            .map(|entry| decided(names, entry)).collect::<Vec<_>>(),
        "withdrawn": decision["withdrawn"].as_array().unwrap().iter()
            .map(|id| names.name(id)).collect::<Vec<_>>(),
    })
}

/// One document entry with its review summary and each generated id named.
fn entry(names: &mut Names, entry: &Value) -> Value {
    let kind = entry["kind"].as_str().unwrap();
    match kind {
        "member" | "reference" => json!([
            kind,
            entry["node"]["id"],
            entry["review"]["outcome"],
            names.name(&entry["review"]["pending_proposal_id"]),
            entry["review"]["comment_count"],
        ]),
        other => json!([other]),
    }
}

/// Every page of the document at one page size, in page order.
async fn pages(host: &StatementHost, names: &mut Names, exclude: bool, limit: usize) -> Value {
    let mut cursor: Option<String> = None;
    let mut pages = Vec::new();
    loop {
        let mut path =
            format!("/requirements/req_shared/document?exclude_terminal={exclude}&limit={limit}");
        if let Some(cursor) = &cursor {
            path.push_str("&cursor=");
            path.push_str(&encode(cursor));
        }
        let (status, body) = call(host, "GET", &path, None).await;
        assert_eq!(status, 200, "{path}: {body}");
        let page = &body["data"];
        pages.push(json!({
            "totals": page["review_totals"],
            "has_more": body["meta"]["has_more"],
            "entries": page["entries"].as_array().unwrap().iter()
                .map(|value| entry(names, value)).collect::<Vec<_>>(),
        }));
        cursor = body["meta"]["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            return json!(pages);
        }
    }
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                char::from(byte).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// Implementation aid: characterization of the review state and the review
/// document before the decision index change. No Rule binds it.
#[tokio::test]
async fn review_state_and_document_pages_keep_their_answers() {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let host = host(&repo, true);
    let mut names = Names::default();
    create_records(&host, &mut names).await;
    review(&host, &mut names).await;

    let mut states = serde_json::Map::new();
    for path in [
        "/requirements/req_shared",
        "/requirements/req_accepted",
        "/requirements/req_rejected",
        "/requirements/req_revised",
        "/requirements/req_resubmitted",
        "/requirements/req_withdrawn",
        "/requirements/req_returned",
        "/requirements/req_waiting",
        "/requirements/req_outside",
        "/resolutions/resolution_choice",
        "/rules/rule_shared",
    ] {
        let record = ok(&host, "GET", path, None).await;
        states.insert(path.into(), decision_state(&mut names, &record["decision"]));
    }
    let mut documents = serde_json::Map::new();
    for exclude in [false, true] {
        for limit in [2, 50] {
            documents.insert(
                format!("exclude_terminal={exclude}&limit={limit}"),
                pages(&host, &mut names, exclude, limit).await,
            );
        }
    }
    let observed = json!({"states": states, "documents": documents});
    let expected: Value = serde_json::from_str(EXPECTED).unwrap();
    assert_eq!(
        observed,
        expected,
        "observed:\n{}",
        serde_json::to_string_pretty(&observed).unwrap()
    );
}

/// The answers observed on the base commit of the decision index change.
const EXPECTED: &str = include_str!("fixtures/document_review_characterization.json");
