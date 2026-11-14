//! The operations that the review page calls, apart from record history,
//! run in one order on one host: for each of the six record kinds the list,
//! the record, the Discussions and their messages, a new Discussion, a
//! reply, a status change, a withdrawal and a decision; then the creation
//! and the change of a Requirement, the document of a Requirement and the
//! repository metadata. That is 58 operations, 25 of them reads.
//!
//! Each call keeps its whole answer: the status, the headers and the body
//! bytes. The pinned form of a run keeps the status, the headers apart from
//! the length, and the body without the projection stamp, with a cursor as
//! whether a next page exists and with the working tree as `<repo>`. Each
//! generated value is named by its place.

use super::names::Names;
use super::router_answer::{Answer, Http};
use super::views::{named, stable_view};
use serde_json::{json, Map, Value};

/// The calls of one run, with their answers.
pub struct Transcript {
    /// The working tree of the repository behind the host.
    pub root: String,
    pub answers: Vec<Answer>,
}

impl Transcript {
    async fn call(
        &mut self,
        http: &Http,
        method: &str,
        path: &str,
        data: Option<Value>,
        headers: &[(&str, &str)],
        shown: &str,
    ) -> Value {
        let answer = http
            .call(method, path, data, headers, format!("{method} {shown}"))
            .await;
        let body = answer.json();
        self.answers.push(answer);
        body
    }

    async fn get(&mut self, http: &Http, path: &str, shown: &str) -> Value {
        self.call(http, "GET", path, None, &[], shown).await
    }

    /// The pinned form of the run: the stable view of the answers with
    /// each generated value named.
    pub fn pinned(&self) -> Value {
        let calls: Vec<Value> = self.answers.iter().map(pinned_call).collect();
        let text = serde_json::to_string(&calls)
            .unwrap()
            .replace(&self.root, "<repo>");
        stable_view(&named(
            &mut Names::default(),
            &serde_json::from_str(&text).unwrap(),
        ))
    }
}

/// One answer in the pinned form. An entity tag is kept without its quotes,
/// so that a generated value in it is named.
fn pinned_call(answer: &Answer) -> Value {
    let mut body = answer.json();
    if let Some(meta) = body.get_mut("meta").and_then(Value::as_object_mut) {
        meta.remove("stamp");
        if let Some(cursor) = meta.remove("next_cursor") {
            meta.insert("next_cursor".into(), json!(!cursor.is_null()));
        }
    }
    let headers: Map<String, Value> = answer
        .headers
        .iter()
        .filter(|(name, _)| name != "content-length")
        .map(|(name, value)| (name.clone(), json!(value.trim_matches('"'))))
        .collect();
    json!({"call": answer.shown, "status": answer.status, "headers": headers, "body": body})
}

/// The pinned form of a run over the fixture repository, when the router
/// runs each operation itself.
const PINNED: &str = include_str!("../fixtures/site_operations.json");

/// Asserts that `pinned` equals the checked transcript. A failure shows each
/// line that differs, then the whole observed transcript.
pub fn assert_pinned(pinned: &Value) {
    let observed = serde_json::to_string_pretty(pinned).unwrap();
    let expected = PINNED.trim_end();
    if observed == expected {
        return;
    }
    let observed_lines: Vec<&str> = observed.lines().collect();
    let expected_lines: Vec<&str> = expected.lines().collect();
    let mut diff = Vec::new();
    for index in 0..observed_lines.len().max(expected_lines.len()) {
        let (left, right) = (expected_lines.get(index), observed_lines.get(index));
        if left != right {
            diff.push(format!("line {}:", index + 1));
            diff.extend(left.map(|line| format!("- {line}")));
            diff.extend(right.map(|line| format!("+ {line}")));
        }
    }
    panic!(
        "the transcript differs from tests/fixtures/site_operations.json \
         (- expected, + observed):\n{}\n=== observed transcript\n{observed}\n=== end",
        diff.join("\n")
    );
}

/// The record kinds, with the id of the record of each kind that the
/// fixture repository holds after `all_kinds`.
const KINDS: [(&str, &str, &str); 6] = [
    ("sources", "source", "source_shared"),
    ("requirements", "requirement", "req_shared"),
    ("resolutions", "resolution", "resolution_shared"),
    ("rules", "rule", "rule_shared"),
    ("topics", "topic", "topic_shared"),
    ("questions", "question", "question_shared"),
];

/// The proposal of the pending submission of the record at `path`, or a
/// proposal id that no submission has.
fn pending(record: &Value) -> String {
    record["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap_or("proposal_missing")
        .to_owned()
}

/// Runs the operations of the review page on `http`. `root` is the
/// working tree of the repository behind it.
pub async fn run(http: &Http, root: &str) -> Transcript {
    let mut out = Transcript {
        root: root.to_owned(),
        answers: Vec::new(),
    };
    for (plural, singular, id) in KINDS {
        kind(&mut out, http, plural, singular, id).await;
    }
    requirement_writes(&mut out, http).await;
    out.get(
        http,
        "/requirements/req_shared/document",
        "/requirements/req_shared/document",
    )
    .await;
    out.call(http, "GET", "/metadata", None, &[], "/metadata")
        .await;
    for (plural, _, id) in KINDS {
        out.get(http, &format!("/{plural}/{id}"), &format!("/{plural}/{id}"))
            .await;
    }
    out
}

async fn kind(out: &mut Transcript, http: &Http, plural: &str, singular: &str, id: &str) {
    let record = format!("/{plural}/{id}");
    let discussions = format!("{record}/discussions");
    out.get(http, &format!("/{plural}"), &format!("/{plural}"))
        .await;
    let read = out.get(http, &record, &record).await;
    let started = out
        .call(
            http,
            "POST",
            &discussions,
            Some(
                json!({"actor":"reviewer", "declared_by":null, "role":"user",
                "body":format!("Discussion of {id}.")}),
            ),
            &[],
            &discussions,
        )
        .await;
    let discussion = started["data"]["discussion_id"]
        .as_str()
        .unwrap_or("discussion_missing")
        .to_owned();
    let thread = format!("{discussions}/{discussion}");
    let shown = format!("{discussions}/{{discussion}}");
    out.get(http, &discussions, &discussions).await;
    out.call(
        http,
        "POST",
        &format!("{thread}/messages"),
        Some(
            json!({"actor":"agent", "declared_by":null, "role":"assistant",
            "body":format!("Reply on {id}.")}),
        ),
        &[("if-match", "\"1\"")],
        &format!("{shown}/messages"),
    )
    .await;
    out.get(
        http,
        &format!("{thread}/messages"),
        &format!("{shown}/messages"),
    )
    .await;
    out.call(
        http,
        "PATCH",
        &thread,
        Some(json!({"actor":"reviewer", "declared_by":null, "status":"resolved"})),
        &[("if-match", "\"2\"")],
        &shown,
    )
    .await;
    let proposal = pending(&read);
    let submission = format!("{record}/submissions/{proposal}");
    let shown = format!("{record}/submissions/{{proposal}}");
    out.call(
        http,
        "POST",
        &format!("{submission}/withdraw"),
        Some(json!({"actor":"agent", "declared_by":null, "reason":"Revise it."})),
        &[],
        &format!("{shown}/withdraw"),
    )
    .await;
    out.call(
        http,
        "POST",
        &format!("{submission}/decide"),
        Some(decision(singular, id)),
        &[],
        &format!("{shown}/decide"),
    )
    .await;
}

fn decision(singular: &str, id: &str) -> Value {
    json!({"actor":{"identity_type":"human","id":"reviewer"},
        "decision":"accepted", "rationale":null,
        "canonical_artifact":{"artifact_type":singular,"artifact_id":id},
        "feedback":null, "declared_by":null})
}

/// Creates a Requirement and withdraws its submission, then changes it and
/// decides the new submission.
// `{proposal}` in a shown call is a placeholder for the generated id.
async fn requirement_writes(out: &mut Transcript, http: &Http) {
    let record = "/requirements/req_new";
    let created = out
        .call(
            http,
            "POST",
            "/requirements",
            Some(json!({"actor":"agent", "id":"req_new",
            "statement":"The record req_new is readable.", "status":"discovery",
            "refines":["req_shared"], "depends_on":[], "supersedes":[]})),
            &[],
            "/requirements",
        )
        .await;
    let proposal = created["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap_or("proposal_missing");
    out.call(
        http,
        "POST",
        &format!("/requirements/req_new/submissions/{proposal}/withdraw"),
        Some(json!({"actor":"agent", "declared_by":null, "reason":"Revise it."})),
        &[],
        &format!("{record}/submissions/{{proposal}}/withdraw"),
    )
    .await;
    let read = out
        .get(http, "/requirements/req_new", "/requirements/req_new")
        .await;
    let etag = read["data"]["edit"]["etag"]
        .as_str()
        .unwrap_or("missing")
        .to_owned();
    let saved = out
        .call(
            http,
            "PATCH",
            "/requirements/req_new",
            Some(json!({"actor":"agent", "statement":"The record req_new is new."})),
            &[("if-match", &etag)],
            "/requirements/req_new",
        )
        .await;
    let proposal = saved["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap_or("proposal_missing");
    out.call(
        http,
        "POST",
        &format!("/requirements/req_new/submissions/{proposal}/decide"),
        Some(decision("requirement", "req_new")),
        &[],
        &format!("{record}/submissions/{{proposal}}/decide"),
    )
    .await;
}
