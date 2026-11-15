//! The requests of the history operations: one case for each history
//! operation of each reviewed kind, as the contract routes serve them.
//!
//! Each case reads the history of its records in the scope `default`: the
//! pages, with `limit=1` and each cursor, each version and an entry id that
//! names no commit, and the evidence after and before each version, of the
//! whole record and of one content field, and at an offset past its end.
//! Each list case also reads the history of the record `absent`, which the
//! scope `default` does not hold. The Requirement cases also read `req_b`
//! in the scope `broken`, whose Requirement file was not UTF-8 at one
//! commit.
//!
//! The store tests run the cases through the operations, and the transport
//! tests through the contract routes, so both read the same requests.

use serde_json::{json, Value};
use std::future::Future;
use std::pin::Pin;

/// One history operation of one reviewed kind: the operation, the record
/// kind, the records of `default` to read and the record of `broken`.
pub struct Case {
    pub operation: &'static str,
    pub kind: &'static str,
    pub records: &'static [&'static str],
    pub broken: Option<&'static str>,
}

const fn case(
    operation: &'static str,
    kind: &'static str,
    records: &'static [&'static str],
    broken: Option<&'static str>,
) -> Case {
    Case {
        operation,
        kind,
        records,
        broken,
    }
}

pub const LIST: &str = "review-history";
pub const ENTRY: &str = "review-history-entry";
pub const EVIDENCE: &str = "review-evidence";
/// A record id that no scope holds.
const ABSENT: &str = "absent";

/// The history operations, one row for each route of the contract.
pub const CASES: [Case; 18] = [
    case(LIST, "requirement", &["req_a"], Some("req_b")),
    case(ENTRY, "requirement", &["req_a"], Some("req_b")),
    case(EVIDENCE, "requirement", &["req_a"], Some("req_b")),
    case(LIST, "rule", &["rule_a"], None),
    case(ENTRY, "rule", &["rule_a"], None),
    case(EVIDENCE, "rule", &["rule_a"], None),
    case(LIST, "source", &["source_a", "source_gone"], None),
    case(ENTRY, "source", &["source_a", "source_gone"], None),
    case(EVIDENCE, "source", &["source_a", "source_gone"], None),
    case(LIST, "topic", &["topic_a"], None),
    case(ENTRY, "topic", &["topic_a"], None),
    case(EVIDENCE, "topic", &["topic_a"], None),
    case(LIST, "question", &["question_a"], None),
    case(ENTRY, "question", &["question_a"], None),
    case(EVIDENCE, "question", &["question_a"], None),
    case(LIST, "resolution", &["resolution_a"], None),
    case(ENTRY, "resolution", &["resolution_a"], None),
    case(EVIDENCE, "resolution", &["resolution_a"], None),
];

/// The content field of a kind that each evidence case reads.
pub fn field(kind: &str) -> &'static str {
    match kind {
        "source" => "name",
        "topic" => "title",
        "question" => "question",
        "resolution" => "position",
        _ => "statement",
    }
}

/// Runs one history operation in a scope, and gives its answer: the
/// success, or the status and body of the failure.
pub trait Reads: Sync {
    fn read<'a>(
        &'a self,
        scope: &'a str,
        operation: &'static str,
        request: Value,
    ) -> Pin<Box<dyn Future<Output = Value> + Send + 'a>>;
}

/// The answer without its stamp, and with a page cursor as whether there
/// is one.
fn shown(mut answer: Value) -> Value {
    if let Some(fields) = answer.as_object_mut() {
        fields.remove("stamp");
    }
    if let Some(cursor) = answer.pointer_mut("/result/next_cursor") {
        *cursor = json!(!cursor.is_null());
    }
    answer
}

/// The version ids of `id` in `scope`, read with `reads`.
async fn versions(reads: &dyn Reads, scope: &str, kind: &str, id: &str) -> Vec<String> {
    let answer = reads
        .read(
            scope,
            LIST,
            json!({"record_kind": kind, "record_id": id, "limit": 200}),
        )
        .await;
    answer
        .pointer("/result/entries")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |entries| {
            entries
                .iter()
                .map(|entry| entry["id"].as_str().unwrap().to_owned())
                .collect()
        })
}

/// The requests of one case for the record `id`.
async fn requests(reads: &dyn Reads, case: &Case, scope: &str, id: &str) -> Vec<Value> {
    let record = json!({"record_kind": case.kind, "record_id": id});
    let with = |fields: Value| {
        let mut request = record.clone();
        request
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        request
    };
    if case.operation == LIST {
        return vec![record.clone(), with(json!({"limit": 1}))];
    }
    let mut entries = versions(reads, scope, case.kind, id).await;
    entries.push("missing".to_owned());
    let mut requests = Vec::new();
    for entry in &entries {
        if case.operation == ENTRY {
            requests.push(with(json!({"entry_id": entry})));
            continue;
        }
        for side in ["after", "before"] {
            requests.push(with(json!({"entry_id": entry, "side": side})));
            requests.push(with(
                json!({"entry_id": entry, "side": side, "field": field(case.kind)}),
            ));
        }
        requests.push(with(
            json!({"entry_id": entry, "side": "after", "offset": 100_000}),
        ));
    }
    requests
}

/// The answers of `requests` of `operation` in `scope`, in their order. A
/// list request follows the cursor of each page that has one.
pub async fn answers(
    reads: &dyn Reads,
    operation: &'static str,
    scope: &str,
    requests: Vec<Value>,
) -> Vec<Value> {
    let mut answers = Vec::new();
    for mut request in requests {
        loop {
            let answer = reads.read(scope, operation, request.clone()).await;
            let next = answer
                .pointer("/result/next_cursor")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let mut sent = request.clone();
            if sent.get("cursor").is_some() {
                sent["cursor"] = json!("<cursor>");
            }
            answers.push(json!({
                "call": format!("{operation} {scope}"),
                "request": sent,
                "answer": shown(answer),
            }));
            let Some(next) = next else { break };
            request["cursor"] = json!(next);
        }
    }
    answers
}

/// The answers of each case of `cases`, in their order.
pub async fn run(reads: &dyn Reads, cases: &[Case]) -> Vec<Value> {
    let mut all = Vec::new();
    for case in cases {
        let records = case.records.iter().map(|id| ("default", *id));
        for (scope, id) in records.chain(case.broken.map(|id| ("broken", id))) {
            let requests = requests(reads, case, scope, id).await;
            all.extend(answers(reads, case.operation, scope, requests).await);
        }
        if case.operation == LIST {
            let request = json!({"record_kind": case.kind, "record_id": ABSENT});
            all.extend(answers(reads, LIST, "default", vec![request]).await);
        }
    }
    all
}
