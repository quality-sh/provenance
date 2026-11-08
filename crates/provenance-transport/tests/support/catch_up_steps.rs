//! The writes and hand edits of the catch-up characterization. Each step
//! runs through one way in: HTTP, the CLI or MCP. The read after each step
//! runs the projection catch-up.

use serde_json::{json, Value};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

/// The status and the body of one call. A status of 200 is a success.
pub type Answer<'a> = Pin<Box<dyn Future<Output = (u16, Value)> + 'a>>;

/// One way in to the public API.
pub trait Surface {
    /// Calls `path`, which can hold a query. `data` is the request data
    /// without the `{"data": ...}` wrapper.
    fn call<'a>(
        &'a self,
        method: &'a str,
        path: &'a str,
        data: Option<Value>,
        headers: &'a [(&'a str, &'a str)],
    ) -> Answer<'a>;
}

/// The number of steps.
pub const STEPS: usize = 23;

async fn ok(surface: &impl Surface, method: &str, path: &str, data: Value) -> Value {
    let (status, value) = surface.call(method, path, Some(data), &[]).await;
    assert_eq!(status, 200, "{method} {path}: {value}");
    value["data"].clone()
}

async fn patch(surface: &impl Surface, path: &str, data: Value) {
    let (status, before) = surface.call("GET", path, None, &[]).await;
    assert_eq!(status, 200, "{path}: {before}");
    let etag = before["data"]["edit"]["etag"].as_str().unwrap().to_owned();
    let (status, saved) = surface
        .call("PATCH", path, Some(data), &[("If-Match", etag.as_str())])
        .await;
    assert_eq!(status, 200, "PATCH {path}: {saved}");
}

async fn pending(surface: &impl Surface, path: &str) -> String {
    let (status, record) = surface.call("GET", path, None, &[]).await;
    assert_eq!(status, 200, "{path}: {record}");
    record["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap_or_else(|| panic!("{path} has no pending submission: {record}"))
        .to_owned()
}

/// One step. Steps 0 to 14 each make one write through `surface`: a new
/// record and its review submission, a relation that comes and goes, a
/// title only, a decision, a withdrawal and a Discussion. The later steps
/// make two writes, or edit the files below `state` by hand.
pub async fn write(surface: &impl Surface, state: &Path, step: usize) {
    if step <= 7 {
        create(surface, step).await;
    } else if step <= 16 {
        change(surface, step).await;
    } else {
        hand_edit(&state.join("scopes/default"), step);
    }
}

/// The steps that create one record each.
async fn create(surface: &impl Surface, step: usize) {
    let (path, data) = match step {
        0 => return,
        1 => (
            "/requirements",
            json!({"actor":"agent", "id":"req_a",
            "statement":"The record req_a is readable.", "status":"discovery",
            "refines":["req_shared"], "depends_on":[], "supersedes":[]}),
        ),
        2 => (
            "/requirements",
            json!({"actor":"agent", "id":"req_b",
            "statement":"The record req_b is readable.", "status":"discovery",
            "refines":["req_shared"], "depends_on":["req_a"], "supersedes":[]}),
        ),
        3 => (
            "/sources",
            json!({"id":"source_a", "name":"Source A",
            "source_type":"document", "url":null, "reference":"docs/a.md",
            "commit_pin":null, "supersedes":[], "origin_thread":null,
            "origin_message":null}),
        ),
        4 => (
            "/resolutions",
            json!({"id":"res_a", "title":"Decision A",
            "position":"Use the record.", "rationale":"The record is available.",
            "status":"draft", "requirement_ids":["req_a"], "supersedes":[],
            "inputs":[]}),
        ),
        5 => (
            "/rules",
            json!({"id":"rule_a",
            "statement":"The host keeps each record.", "status":"active",
            "severity":"high", "requirement_ids":["req_a"], "resolution_ids":[]}),
        ),
        6 => (
            "/topics",
            json!({"id":"topic_a", "requirement_id":"req_a",
            "title":"Topic A", "status":"open",
            "links":[{"target_type":"source","target_id":"source_a"}]}),
        ),
        7 => (
            "/questions",
            json!({"id":"q_a", "topic_id":"topic_a",
            "question":"Which record is first?", "resolution_method":"research",
            "status":"open", "links":[{"target_type":"rule","target_id":"rule_a"}]}),
        ),
        _ => unreachable!("no create step {step}"),
    };
    ok(surface, "POST", path, data).await;
}

/// The steps that change or decide a record, or start a Discussion.
async fn change(surface: &impl Surface, step: usize) {
    match step {
        8 => {
            patch(
                surface,
                "/topics/topic_a",
                json!({"title":"Topic A, renamed"}),
            )
            .await
        }
        9 => {
            patch(
                surface,
                "/requirements/req_b",
                json!({"actor":"agent", "relationships":{"depends_on":{"remove":["req_a"]}}}),
            )
            .await;
        }
        10 => {
            patch(
                surface,
                "/requirements/req_b",
                json!({"actor":"agent", "statement":"The record req_b is new."}),
            )
            .await;
        }
        11 => {
            let proposal = pending(surface, "/requirements/req_a").await;
            ok(
                surface,
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
            let proposal = pending(surface, "/requirements/req_b").await;
            ok(
                surface,
                "POST",
                &format!("/requirements/req_b/submissions/{proposal}/withdraw"),
                json!({"actor":"agent", "declared_by":null, "reason":"Revise it."}),
            )
            .await;
        }
        13 => patch(surface, "/questions/q_a", json!({"links":[]})).await,
        14 => discuss(surface, "req_a", "Name the scope.").await,
        // Two writes in two directories before one read.
        15 => {
            patch(
                surface,
                "/topics/topic_a",
                json!({"title":"Topic A, again"}),
            )
            .await;
            patch(
                surface,
                "/requirements/req_b",
                json!({"actor":"agent", "statement":"The record req_b is newer."}),
            )
            .await;
        }
        16 => discuss(surface, "req_b", "Keep this record.").await,
        _ => unreachable!("no change step {step}"),
    }
}

async fn discuss(surface: &impl Surface, requirement: &str, body: &str) {
    ok(
        surface,
        "POST",
        &format!("/requirements/{requirement}/discussions"),
        json!({"actor":"reviewer", "declared_by":null, "role":"user", "body":body}),
    )
    .await;
}

/// The steps that change the files of the scope directory `scope` by hand.
fn hand_edit(scope: &Path, step: usize) {
    let requirements = scope.join("requirements/req.jsonl");
    let parked = scope.join("notes/req_b.jsonl");
    match step {
        // The parent of a Discussion goes: req_b moves out of the
        // requirements file to a directory that no family reads.
        17 => {
            let text = std::fs::read_to_string(&requirements).unwrap();
            let (moved, kept): (Vec<&str>, Vec<&str>) = text
                .lines()
                .partition(|line| line.contains("\"id\":\"req_b\""));
            assert_eq!(moved.len(), 1, "{text}");
            std::fs::create_dir_all(parked.parent().unwrap()).unwrap();
            std::fs::write(&parked, format!("{}\n", moved[0])).unwrap();
            std::fs::write(&requirements, lines(&kept)).unwrap();
        }
        // The parent comes back, and the notes file is deleted.
        18 => {
            let mut text = std::fs::read_to_string(&requirements).unwrap();
            text.push_str(&std::fs::read_to_string(&parked).unwrap());
            std::fs::write(&requirements, text).unwrap();
            std::fs::remove_file(&parked).unwrap();
        }
        // The rules file moves to the topics directory, and back.
        19 => std::fs::rename(
            scope.join("rules/rule.jsonl"),
            scope.join("topics/rule.jsonl"),
        )
        .unwrap(),
        20 => std::fs::rename(
            scope.join("topics/rule.jsonl"),
            scope.join("rules/rule.jsonl"),
        )
        .unwrap(),
        // Files outside every named directory, and at the top of the scope.
        21 => {
            std::fs::write(scope.join("notes/note.jsonl"), "{\"note\":\"one\"}\n").unwrap();
            std::fs::write(scope.join("loose.jsonl"), "{\"note\":\"two\"}\n").unwrap();
        }
        // A deleted file, not an emptied one.
        22 => std::fs::remove_file(scope.join("rules/rule.jsonl")).unwrap(),
        _ => unreachable!("no hand edit step {step}"),
    }
}

fn lines(kept: &[&str]) -> String {
    kept.iter().map(|line| format!("{line}\n")).collect()
}
