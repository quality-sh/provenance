//! The history operations on one Git history: a merge, a Discussion
//! outcome committed before its record, a deleted record, a renamed record
//! file, a record file that was not UTF-8 at one commit, and an entry id
//! that names no commit.
//!
//! `CASES` holds one row for each history operation of each reviewed kind,
//! as the contract routes serve them. Each row reads the history of its
//! record in the scope `default`, and of a record in the scope `broken`,
//! whose Requirement file was not UTF-8 at one commit. The answers, without
//! their stamps and with each commit id as its name in the graph, equal the
//! checked file over a folder repository with real Git history.
//!
//! Implementation aid with no Rule binding: these tests pin the answers of
//! the history operations so that a change of the path they read through
//! cannot change them.

use crate::history_fixture::{history, History, Sources};
use crate::memory::FixedClock;
use provenance_store::repository::IdSource;
use serde_json::{json, Value};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// The checked answers of the cases.
const PINNED: &str = include_str!("../fixtures/history_operations.json");

/// One history operation of one reviewed kind: the operation, the record
/// kind, the records of `default` to read and the record of `broken`.
pub struct Case {
    pub operation: &'static str,
    pub kind: &'static str,
    pub records: &'static [&'static str],
    pub broken: &'static str,
}

const fn case(
    operation: &'static str,
    kind: &'static str,
    records: &'static [&'static str],
    broken: &'static str,
) -> Case {
    Case {
        operation,
        kind,
        records,
        broken,
    }
}

const LIST: &str = "review-history";
const ENTRY: &str = "review-history-entry";
const EVIDENCE: &str = "review-evidence";

/// The history operations, one row for each route of the contract.
pub const CASES: [Case; 18] = [
    case(LIST, "requirement", &["req_a"], "req_b"),
    case(ENTRY, "requirement", &["req_a"], "req_b"),
    case(EVIDENCE, "requirement", &["req_a"], "req_b"),
    case(LIST, "rule", &["rule_a"], "rule_a"),
    case(ENTRY, "rule", &["rule_a"], "rule_a"),
    case(EVIDENCE, "rule", &["rule_a"], "rule_a"),
    case(LIST, "source", &["source_a", "source_gone"], "source_a"),
    case(ENTRY, "source", &["source_a", "source_gone"], "source_a"),
    case(EVIDENCE, "source", &["source_a", "source_gone"], "source_a"),
    case(LIST, "topic", &["topic_a"], "topic_a"),
    case(ENTRY, "topic", &["topic_a"], "topic_a"),
    case(EVIDENCE, "topic", &["topic_a"], "topic_a"),
    case(LIST, "question", &["question_a"], "question_a"),
    case(ENTRY, "question", &["question_a"], "question_a"),
    case(EVIDENCE, "question", &["question_a"], "question_a"),
    case(LIST, "resolution", &["resolution_a"], "resolution_a"),
    case(ENTRY, "resolution", &["resolution_a"], "resolution_a"),
    case(EVIDENCE, "resolution", &["resolution_a"], "resolution_a"),
];

/// The content field of a kind that each evidence case reads.
fn field(kind: &str) -> &'static str {
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
pub trait Reads {
    fn read<'a>(
        &'a self,
        scope: &'a str,
        operation: &'static str,
        request: Value,
    ) -> Pin<Box<dyn Future<Output = Value> + 'a>>;
}

/// Ids made from a counter, so each run makes the same ids.
#[derive(Debug, Default)]
struct Counted(AtomicU64);

impl IdSource for Counted {
    fn new_uuid(&self) -> uuid::Uuid {
        uuid::Uuid::from_u128(u128::from(self.0.fetch_add(1, Ordering::SeqCst) + 1))
    }
}

/// The history, with fixed times and ids.
pub fn fixture() -> History {
    history(&Sources {
        clock: Arc::new(FixedClock::at_millis(1_700_000_000_000)),
        ids: Arc::new(Counted::default()),
    })
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
        .read(scope, LIST, json!({"record_kind": kind, "record_id": id, "limit": 200}))
        .await;
    answer
        .pointer("/result/entries")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .map(|entry| entry["id"].as_str().unwrap().to_owned())
                .collect()
        })
        .unwrap_or_default()
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
        requests.push(with(json!({"entry_id": entry, "side": "after", "offset": 100_000})));
    }
    requests
}

/// The answers of each case, in the order of `CASES`. A list case follows
/// the cursor of each page that has one.
pub async fn run(reads: &dyn Reads) -> Vec<Value> {
    let mut answers = Vec::new();
    for case in &CASES {
        let records = case.records.iter().map(|id| ("default", *id));
        for (scope, id) in records.chain([("broken", case.broken)]) {
            for request in requests(reads, case, scope, id).await {
                let mut request = request;
                loop {
                    let answer = reads.read(scope, case.operation, request.clone()).await;
                    let next = answer
                        .pointer("/result/next_cursor")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    answers.push(json!({
                        "call": format!("{} {scope}", case.operation),
                        "request": request,
                        "answer": shown(answer),
                    }));
                    let Some(next) = next else { break };
                    request["cursor"] = json!(next);
                }
            }
        }
    }
    answers
}

/// Fails with each line of `observed` that differs from the checked file,
/// and with the whole observed text.
pub fn assert_pinned(observed: &str) {
    let expected = PINNED.trim_end();
    if observed == expected {
        return;
    }
    let (observed_lines, expected_lines): (Vec<_>, Vec<_>) =
        (observed.lines().collect(), expected.lines().collect());
    let mut diff = Vec::new();
    for index in 0..observed_lines.len().max(expected_lines.len()) {
        let (left, right) = (expected_lines.get(index), observed_lines.get(index));
        if left != right {
            diff.push(format!("line {}:", index + 1));
            diff.extend(left.map(|line| format!("- {line}")));
            diff.extend(right.map(|line| format!("+ {line}")));
        }
        if diff.len() > 60 {
            break;
        }
    }
    panic!(
        "the answers differ from the checked file:\n{}\n=== OBSERVED\n{observed}\n=== END",
        diff.join("\n")
    );
}

#[cfg(feature = "host")]
mod folder {
    use super::{assert_pinned, fixture, run, Reads};
    use crate::history_folder::folder;
    use provenance_core::protocol::repository::RepositoryContext;
    use provenance_store::operations::catalog::{
        frame_failure, Direct, Invoke as _, Operation, RequestedContext, ReviewEvidence,
        ReviewHistory, ReviewHistoryEntry,
    };
    use provenance_store::operations::LocalRepository;
    use provenance_store::repository::Repository;
    use serde_json::{json, Value};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;

    /// Runs each operation on a folder repository as the contract dispatch
    /// runs it on the host, with no port.
    struct FolderReads(Arc<LocalRepository>);

    async fn direct<O: Operation>(binding: Arc<LocalRepository>, scope: &str, request: Value) -> Value {
        let context: RepositoryContext =
            serde_json::from_value(json!({"repository": "memory", "scope": scope})).unwrap();
        let request: O::Request = serde_json::from_value(request).unwrap();
        match Direct(binding)
            .invoke::<O>(Some(RequestedContext::Scoped(context)), request)
            .await
        {
            Ok(success) => serde_json::to_value(success).unwrap(),
            Err(failure) => {
                let failure = frame_failure::<O>(failure);
                json!({"status": failure.status_code(), "error": failure.error})
            }
        }
    }

    impl Reads for FolderReads {
        fn read<'a>(
            &'a self,
            scope: &'a str,
            operation: &'static str,
            request: Value,
        ) -> Pin<Box<dyn Future<Output = Value> + 'a>> {
            let binding = Arc::clone(&self.0);
            match operation {
                "review-history" => Box::pin(direct::<ReviewHistory>(binding, scope, request)),
                "review-history-entry" => {
                    Box::pin(direct::<ReviewHistoryEntry>(binding, scope, request))
                }
                "review-evidence" => Box::pin(direct::<ReviewEvidence>(binding, scope, request)),
                other => panic!("{other} is not a history operation"),
            }
        }
    }

    #[tokio::test]
    async fn the_history_operations_over_a_folder_give_the_checked_answers() {
        let folder = folder(&fixture());
        let repository = Repository::open(folder.root(), "default").unwrap();
        let binding = Arc::new(LocalRepository::bind(repository, "memory").unwrap());
        let answers = run(&FolderReads(binding)).await;
        let text = serde_json::to_string_pretty(&answers).unwrap();
        assert_pinned(&folder.named(&text));
    }
}
