#[path = "query_support/fixtures.rs"]
mod fixtures;

use fixtures::{apply_shared_rule, init_repo, sdk, sdk_error};
use serde_json::{json, Value};

fn at_depth(answer: &Value, depth: u64) -> Vec<String> {
    let mut ids = answer["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|reached| reached["depth"] == depth)
        .map(|reached| reached["node"]["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

/// A source is named by the requirements that cite it, and each of those
/// by the rule's list, so the walk to the rules reads `in` at every hop.
#[test]
fn trace_walks_from_a_source_to_the_rules_it_grounds() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);

    let answer = sdk(
        repo,
        "trace",
        &json!({"id": ids.source.as_str(), "direction": "in"}),
    );

    assert_eq!(
        answer["protocol_version"],
        provenance_core::SDK_PROTOCOL_VERSION
    );
    assert_eq!(answer["operation"], "trace");
    assert_eq!(answer["id"], ids.source);
    assert_eq!(answer["max_depth"], 3);
    let mut requirements = vec![ids.sharing, ids.sessions];
    requirements.sort();
    assert_eq!(at_depth(&answer, 1), requirements);
    assert_eq!(at_depth(&answer, 2), vec![ids.rule]);
}

#[test]
fn trace_stops_at_the_depth_the_caller_names() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);

    let answer = sdk(
        repo,
        "trace",
        &json!({"id": ids.source, "direction": "in", "max_depth": 1}),
    );

    assert_eq!(answer["max_depth"], 1);
    assert_eq!(answer["nodes"].as_array().unwrap().len(), 2);
    assert!(at_depth(&answer, 2).is_empty());
}

/// The rule's own list names its requirements, and each requirement's own
/// citation names the source: the walk back reads `out` at every hop.
#[test]
fn trace_walks_back_from_a_rule_to_the_source_behind_it() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);

    let answer = sdk(repo, "trace", &json!({"id": ids.rule, "direction": "out"}));

    assert_eq!(at_depth(&answer, 2), vec![ids.source]);
}

#[test]
fn trace_refuses_a_walk_deeper_than_the_engine_will_bound() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);

    let error = sdk_error(repo, "trace", &json!({"id": ids.source, "max_depth": 99}));

    assert!(
        error.contains("max_depth must be between 1 and 10"),
        "error should name the cap: {error}"
    );
}
