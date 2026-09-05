//! The golden test: a fixed request set over the frozen corpus answers
//! the same bytes as the committed file, keyed by `READ_DERIVATION`.
//!
//! The file is regenerated only in a commit that bumps the constant:
//! `PROVENANCE_GOLDEN_WRITE=1 cargo test -p provenance-store golden`.

use super::differential::corpus::Corpus;
use super::differential::requests::{self, Request};
use super::differential::{served_value, strip_additive};
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::operations::stamp::READ_DERIVATION;
use provenance_core::protocol::{
    GetQuery, ImpactQuery, NeighborsQuery, ResolveSymbolQuery, SDK_PROTOCOL_VERSION,
};
use provenance_core::NodeType;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/operations/queries/tests/golden.json")
}

fn get(kind: NodeType, id: &str, include_retired: bool) -> Request {
    Request::Get(GetQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        node_type: kind,
        id: id.into(),
        include_retired,
    })
}

fn impact(id: &str, include_retired: bool, limit: usize) -> Request {
    Request::Impact(ImpactQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        id: id.into(),
        node_type: None,
        include_retired,
        limit,
    })
}

fn resolve(file: &str, symbol: Option<&str>, line: Option<usize>) -> Request {
    Request::ResolveSymbol(ResolveSymbolQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        file: file.into(),
        symbol: symbol.map(str::to_string),
        line,
        include_retired: false,
        limit: 50,
    })
}

/// The fixed request set. `stale` is absent: its answer names commit ids,
/// which no frozen corpus can hold.
fn request_set() -> Vec<Request> {
    let mut set = vec![
        get(NodeType::Domain, "domain_payroll", false),
        get(NodeType::Source, "source_schads", false),
        get(NodeType::Requirement, "req_overtime", false),
        get(NodeType::Resolution, "res_overtime", false),
        get(NodeType::Rule, "rule_overtime_001", false),
        get(NodeType::Topic, "topic_rates", false),
        get(NodeType::Question, "question_threshold", false),
        get(NodeType::Boundary, "boundary_no_backpay", false),
        get(NodeType::Requirement, "req_old_overtime", false),
        get(NodeType::Requirement, "req_old_overtime", true),
        get(NodeType::Requirement, "twin_record", false),
        get(NodeType::Rule, "twin_record", false),
        Request::Search(requests::search("over", Vec::new())),
        Request::Search(requests::search(
            "pay",
            vec![NodeType::Domain, NodeType::Boundary],
        )),
        Request::Search(requests::search("e", Vec::new())),
    ];
    let mut with_retired = requests::search("over", Vec::new());
    with_retired.include_retired = true;
    set.push(Request::Search(with_retired));
    set.extend([
        Request::Neighbors(requests::neighbors("req_overtime", false, 5)),
        Request::Neighbors(requests::neighbors("req_overtime", true, 50)),
        Request::Neighbors(requests::neighbors("source_schads", false, 50)),
        Request::Neighbors(requests::neighbors("topic_rates", false, 50)),
        Request::Neighbors(requests::neighbors("req_old_overtime", true, 50)),
        Request::Neighbors(requests::neighbors("question_threshold", false, 50)),
        Request::Neighbors(requests::neighbors("twin_record", false, 50)),
    ]);
    let mut retired_origin = requests::neighbors("req_right", false, 50);
    retired_origin.node_type = Some(NodeType::Requirement);
    set.push(Request::Neighbors(retired_origin));
    let mut by_relation = requests::neighbors("req_penalty", false, 50);
    by_relation.relations = vec!["contradicts".into(), "refines".into()];
    set.push(Request::Neighbors(by_relation));
    let mut outward = NeighborsQuery {
        direction: provenance_core::protocol::Direction::Out,
        ..requests::neighbors("req_overtime", false, 50)
    };
    outward.limit = 50;
    set.push(Request::Neighbors(outward));
    set.extend([
        Request::Trace(requests::trace("req_top", false, 50)),
        Request::Trace(requests::trace("req_top", true, 50)),
        Request::Trace(requests::trace("req_overtime", false, 3)),
        Request::Trace(requests::trace("twin_record", false, 50)),
        impact("source_schads", false, 50),
        impact("req_overtime", false, 5),
        impact("rule_overtime_001", false, 50),
        impact("req_top", false, 50),
        impact("req_top", true, 50),
        impact("res_overtime", false, 50),
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
    let mut with_retired = requests::evidence("rule_overtime_001", None);
    with_retired.include_retired = true;
    set.push(Request::Evidence(with_retired));
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
    let corpus = Corpus::golden();
    crate::cache::catch_up_state(&corpus.layout())
        .await
        .unwrap();
    crate::test_probes::set_preset_scan(None);
    let policy = ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly);
    let mut answers = Vec::new();
    for request in request_set() {
        let mut answer = served_value(&corpus, &request, policy).await;
        strip_additive(&mut answer);
        answers.push(json!({
            "operation": request.operation(),
            "request": request.describe(),
            "answer": answer,
        }));
    }
    answers
}

fn digest(answers: &[Value]) -> String {
    let bytes = serde_json::to_vec(answers).unwrap();
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[tokio::test]
async fn the_golden_answers_match_the_committed_file_for_this_derivation() {
    let answers = answers().await;
    let fresh = digest(&answers);
    if std::env::var("PROVENANCE_GOLDEN_WRITE").is_ok_and(|value| value == "1") {
        let file = json!({
            "derivation": READ_DERIVATION,
            "digest": fresh,
            "answers": answers,
        });
        std::fs::write(golden_path(), serde_json::to_string_pretty(&file).unwrap()).unwrap();
        return;
    }
    let stored: Value = serde_json::from_str(
        &std::fs::read_to_string(golden_path()).expect("golden.json is committed beside this test"),
    )
    .unwrap();
    assert_eq!(
        stored["derivation"],
        json!(READ_DERIVATION),
        "the golden file is keyed by READ_DERIVATION; regenerate it in the commit that bumps the constant"
    );
    if stored["digest"] != json!(fresh) {
        let recorded = stored["answers"].as_array().unwrap();
        for (index, answer) in answers.iter().enumerate() {
            assert_eq!(
                recorded.get(index),
                Some(answer),
                "answer {index} differs from the golden file"
            );
        }
        panic!("the golden digest differs although every answer matched");
    }
}
