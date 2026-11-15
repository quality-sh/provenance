//! The 18 history routes of the review page, one row for each, called on
//! one host over the history of `history_fixture.rs`.
//!
//! Each row reads the history of one record of its kind: its pages, each
//! version and an entry id that names no commit, and the evidence after and
//! before each version, of the whole record and of one field, and at an
//! offset past its end. The Source rows also read `source_gone`, which a
//! later commit deleted. Each row then reads one record on a host of the
//! scope `broken`, whose Requirement file was not UTF-8 at one commit.

use super::router_answer::{Answer, Http};
use serde_json::Value;

/// One history route: its operation id in the contract, the records of
/// `default` and the record of `broken` that it reads, and the field of
/// its evidence reads.
pub struct Row {
    pub operation_id: &'static str,
    pub records: &'static [&'static str],
    pub broken: &'static str,
    pub field: &'static str,
}

const fn row(
    operation_id: &'static str,
    records: &'static [&'static str],
    broken: &'static str,
    field: &'static str,
) -> Row {
    Row {
        operation_id,
        records,
        broken,
        field,
    }
}

/// Each history route of the contract.
pub const ROWS: [Row; 18] = [
    row("list-requirement-history", &["req_a"], "req_b", "statement"),
    row(
        "get-requirement-history-entry",
        &["req_a"],
        "req_b",
        "statement",
    ),
    row(
        "get-requirement-history-evidence",
        &["req_a"],
        "req_b",
        "statement",
    ),
    row("list-rule-history", &["rule_a"], "rule_a", "statement"),
    row("get-rule-history-entry", &["rule_a"], "rule_a", "statement"),
    row(
        "get-rule-history-evidence",
        &["rule_a"],
        "rule_a",
        "statement",
    ),
    row(
        "list-source-history",
        &["source_a", "source_gone"],
        "source_a",
        "name",
    ),
    row(
        "get-source-history-entry",
        &["source_a", "source_gone"],
        "source_a",
        "name",
    ),
    row(
        "get-source-history-evidence",
        &["source_a", "source_gone"],
        "source_a",
        "name",
    ),
    row("list-topic-history", &["topic_a"], "topic_a", "title"),
    row("get-topic-history-entry", &["topic_a"], "topic_a", "title"),
    row(
        "get-topic-history-evidence",
        &["topic_a"],
        "topic_a",
        "title",
    ),
    row(
        "list-question-history",
        &["question_a"],
        "question_a",
        "question",
    ),
    row(
        "get-question-history-entry",
        &["question_a"],
        "question_a",
        "question",
    ),
    row(
        "get-question-history-evidence",
        &["question_a"],
        "question_a",
        "question",
    ),
    row(
        "list-resolution-history",
        &["resolution_a"],
        "resolution_a",
        "position",
    ),
    row(
        "get-resolution-history-entry",
        &["resolution_a"],
        "position",
    ),
    row(
        "get-resolution-history-evidence",
        &["resolution_a"],
        "position",
    ),
];

/// The path of the route `operation_id`.
pub fn route(operation_id: &str) -> &'static str {
    provenance_contract::definitions()
        .iter()
        .find(|definition| definition.operation_id == operation_id)
        .unwrap_or_else(|| panic!("no route {operation_id}"))
        .path
}

/// The operation id of each route of the contract whose path reads record
/// history.
pub fn history_routes() -> Vec<&'static str> {
    provenance_contract::definitions()
        .iter()
        .filter(|definition| definition.path.contains("/history"))
        .map(|definition| definition.operation_id)
        .collect()
}

async fn get(http: &Http, path: String) -> Answer {
    http.call("GET", &path, None, &[], format!("GET {path}"))
        .await
}

/// The version ids of the record at the list path `list`.
async fn versions(http: &Http, list: &str) -> Vec<String> {
    let body = get(http, format!("{list}?limit=200")).await.json();
    body.pointer("/data/entries")
        .or_else(|| body.pointer("/data"))
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry["id"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// The paths of one row for the record `id`.
async fn paths(http: &Http, row: &Row, id: &str) -> Vec<String> {
    let template = route(row.operation_id).replace("{id}", id);
    let list = route(row.operation_id)
        .split("/history")
        .next()
        .unwrap()
        .replace("{id}", id)
        + "/history";
    if !template.contains("{entry_id}") {
        return vec![template.clone(), format!("{template}?limit=1")];
    }
    let mut entries = versions(http, &list).await;
    entries.push("missing".to_owned());
    let mut paths = Vec::new();
    for entry in &entries {
        let path = template.replace("{entry_id}", entry);
        if !path.contains("{side}") {
            paths.push(path);
            continue;
        }
        for side in ["after", "before"] {
            let path = path.replace("{side}", side);
            paths.push(format!("{path}?field={}", row.field));
            paths.push(path);
        }
        paths.push(format!("{}?offset=100000", path.replace("{side}", "after")));
    }
    paths
}

/// The answers of each row, in the order of `ROWS`, on the host of
/// `default` and then on the host of `broken`. A list follows the cursor
/// of each page that has one.
pub async fn run(default: &Http, broken: &Http) -> Vec<Answer> {
    let mut answers = Vec::new();
    for row in &ROWS {
        let records = row.records.iter().map(|id| (default, *id));
        for (http, id) in records.chain([(broken, row.broken)]) {
            for path in paths(http, row, id).await {
                let mut path = path;
                loop {
                    let answer = get(http, path.clone()).await;
                    let next = answer
                        .json()
                        .pointer("/meta/next_cursor")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    answers.push(answer);
                    let Some(next) = next else { break };
                    let base = path.split('?').next().unwrap().to_owned();
                    path = format!("{base}?limit=1&cursor={next}");
                }
            }
        }
    }
    answers
}
