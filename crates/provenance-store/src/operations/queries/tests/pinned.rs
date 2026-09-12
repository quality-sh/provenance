//! The pinned answers test: a fixed request set over the frozen store answers
//! the same bytes as the committed file, keyed by `READ_DERIVATION`.
//!
//! The file holds one line for the key and the digest, then one compact
//! line per answer, so a regeneration diff reads answer by answer. It is
//! regenerated only in a commit that bumps the constant:
//! `PROVENANCE_PINNED_WRITE=1 cargo test -p provenance-store pinned`.

use super::comparison::requests::{self, Request};
use super::comparison::test_stores::TestStore;
use super::comparison::{served_value, strip_additive};
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::operations::stamp::READ_DERIVATION;
use provenance_core::protocol::{
    GetQuery, ImpactQuery, NeighborsQuery, ResolveSymbolQuery, StaleQuery, SDK_PROTOCOL_VERSION,
};
use provenance_core::NodeType;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn pinned_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/operations/queries/tests/pinned_answers.json")
}

fn get(kind: NodeType, id: &str) -> Request {
    Request::Get(GetQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        node_type: kind,
        id: id.into(),
    })
}

fn impact(id: &str, limit: usize) -> Request {
    Request::Impact(ImpactQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        id: id.into(),
        node_type: None,

        limit,
    })
}

fn resolve(file: &str, symbol: Option<&str>, line: Option<usize>) -> Request {
    Request::ResolveSymbol(ResolveSymbolQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        file: file.into(),
        symbol: symbol.map(str::to_string),
        line,

        limit: 50,
    })
}

/// The fixed request set. `base` is the store's first commit, whose id
/// is fixed by the store's fixed author and dates.
pub(super) fn request_set(base: &str) -> Vec<Request> {
    let mut set = vec![
        get(NodeType::Domain, "domain_payroll"),
        get(NodeType::Source, "source_schads"),
        get(NodeType::Requirement, "req_overtime"),
        get(NodeType::Resolution, "res_overtime"),
        get(NodeType::Rule, "rule_overtime_001"),
        get(NodeType::Topic, "topic_rates"),
        get(NodeType::Question, "question_threshold"),
        get(NodeType::Boundary, "boundary_no_backpay"),
        get(NodeType::Requirement, "req_old_overtime"),
        get(NodeType::Requirement, "req_old_overtime"),
        get(NodeType::Requirement, "twin_record"),
        get(NodeType::Rule, "twin_record"),
        Request::Search(requests::search("over", Vec::new())),
        Request::Search(requests::search(
            "pay",
            vec![NodeType::Domain, NodeType::Boundary],
        )),
        Request::Search(requests::search("e", Vec::new())),
    ];
    set.extend([
        Request::Neighbors(requests::neighbors("req_overtime", 5)),
        Request::Neighbors(requests::neighbors("req_overtime", 50)),
        Request::Neighbors(requests::neighbors("source_schads", 50)),
        Request::Neighbors(requests::neighbors("topic_rates", 50)),
        Request::Neighbors(requests::neighbors("req_old_overtime", 50)),
        Request::Neighbors(requests::neighbors("question_threshold", 50)),
        Request::Neighbors(requests::neighbors("twin_record", 50)),
    ]);
    let mut named_origin = requests::neighbors("req_right", 50);
    named_origin.node_type = Some(NodeType::Requirement);
    set.push(Request::Neighbors(named_origin));
    let mut by_relation = requests::neighbors("req_penalty", 50);
    by_relation.relations = vec!["contradicts".into(), "refines".into()];
    set.push(Request::Neighbors(by_relation));
    let mut outward = NeighborsQuery {
        direction: provenance_core::protocol::Direction::Out,
        ..requests::neighbors("req_overtime", 50)
    };
    outward.limit = 50;
    set.push(Request::Neighbors(outward));
    set.extend([
        Request::Trace(requests::trace("req_top", 50)),
        Request::Trace(requests::trace("req_top", 50)),
        Request::Trace(requests::trace("req_overtime", 3)),
        Request::Trace(requests::trace("twin_record", 50)),
        impact("source_schads", 50),
        impact("req_overtime", 5),
        impact("rule_overtime_001", 50),
        impact("req_top", 50),
        impact("req_top", 50),
        impact("res_overtime", 50),
    ]);
    let mut short = requests::evidence("rule_overtime_001", None);
    short.limit = 2;
    set.push(Request::Evidence(short));
    set.push(Request::Evidence(requests::evidence(
        "rule_overtime_001",
        None,
    )));
    set.push(Request::Evidence(requests::evidence(
        "rule_penalty_001",
        None,
    )));
    set.push(Request::Evidence(requests::evidence(
        "rule_overtime_001",
        Some(base.to_string()),
    )));
    set.push(Request::Stale(StaleQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        base: base.to_string(),
        head: None,
        rules: Vec::new(),

        limit: 50,
    }));
    set.extend([
        resolve("src/pay.rs", None, None),
        resolve("src/pay.rs", Some("pay"), None),
        resolve("src/pay.rs", None, Some(2)),
        resolve("src/rates.rs", None, None),
        resolve("src/none.rs", None, None),
    ]);
    set
}

async fn answers() -> Vec<Value> {
    let store = TestStore::pinned();
    let base = store.base_commit.clone().expect("git on the path");
    crate::cache::catch_up_state(&store.layout()).await.unwrap();
    crate::test_probes::set_test_scan(None);
    let policy = ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly);
    let mut answers = Vec::new();
    for request in request_set(&base) {
        let mut answer = served_value(&store, &request, policy).await;
        strip_additive(&mut answer);
        normalize_record_stamps(&mut answer);
        normalize_cursor(&mut answer);
        answers.push(json!({
            "operation": request.operation(),
            "request": request.describe(),
            "answer": answer,
        }));
    }
    answers
}

// The pinned file holds record content. Stamp behavior and SQL retention have
// dedicated tests; archive permalinks remain part of the content pinned here.
fn normalize_record_stamps(value: &mut Value) {
    match value {
        Value::Object(record) => {
            if matches!(
                record.get("node_type").and_then(Value::as_str),
                Some("source" | "requirement" | "rule" | "resolution")
            ) {
                record.remove("created");
                record.remove("updated");
            }
            for value in record.values_mut() {
                normalize_record_stamps(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                normalize_record_stamps(value);
            }
        }
        _ => {}
    }
}

// Cursor authentication and target binding have dedicated behavioral tests.
// The golden file retains the revision digest, serial, derivation, and position;
// only random instance, temporary repository identity, and signature vary.
fn normalize_cursor(answer: &mut Value) {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    let Some(token) = answer.get("next_cursor").and_then(Value::as_str) else {
        return;
    };
    let (body, signature) = token.split_once('.').expect("authenticated cursor framing");
    assert_eq!(URL_SAFE_NO_PAD.decode(signature).unwrap().len(), 32);
    let mut payload: Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(body).unwrap()).unwrap();
    assert!(payload["identity"]
        .as_str()
        .is_some_and(|id| id.len() == 64));
    assert!(uuid::Uuid::parse_str(payload["instance"].as_str().unwrap()).is_ok());
    payload["identity"] = json!("<query identity>");
    payload["instance"] = json!("<projection instance>");
    answer["next_cursor"] = payload;
}

fn digest(answers: &[Value]) -> String {
    let bytes = serde_json::to_vec(answers).unwrap();
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// The file: one header line, then one compact line per answer.
fn render(answers: &[Value], digest: &str) -> String {
    let mut lines = vec![json!({ "derivation": READ_DERIVATION, "digest": digest }).to_string()];
    lines.extend(answers.iter().map(Value::to_string));
    format!("{}\n", lines.join("\n"))
}

fn parse(file: &str) -> (Value, Vec<Value>) {
    let mut lines = file
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap());
    let header = lines.next().expect("a header line");
    (header, lines.collect())
}

#[tokio::test]
async fn the_pinned_answers_match_the_committed_file_for_this_derivation() {
    let answers = answers().await;
    let fresh = digest(&answers);
    if std::env::var("PROVENANCE_PINNED_WRITE").is_ok_and(|value| value == "1") {
        std::fs::write(pinned_path(), render(&answers, &fresh)).unwrap();
        eprintln!(
            "PROVENANCE_PINNED_WRITE=1: wrote {} answers to {}",
            answers.len(),
            pinned_path().display()
        );
        assert_eq!(
            answers,
            self::answers().await,
            "fresh fixture repositories must produce identical pinned content"
        );
    }
    let (header, recorded) = parse(
        &std::fs::read_to_string(pinned_path())
            .expect("pinned_answers.json is committed beside this test"),
    );
    assert_eq!(
        header["derivation"],
        json!(READ_DERIVATION),
        "the pinned file is keyed by READ_DERIVATION; regenerate it in the commit that bumps the constant"
    );
    if header["digest"] != json!(fresh) {
        for (index, answer) in answers.iter().enumerate() {
            assert_eq!(
                recorded.get(index),
                Some(answer),
                "answer {index} differs from the pinned file"
            );
        }
        assert_eq!(
            recorded.len(),
            answers.len(),
            "the pinned file has more answers"
        );
    }
}
