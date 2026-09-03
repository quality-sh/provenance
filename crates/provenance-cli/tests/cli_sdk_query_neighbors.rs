#[path = "query_support/fixtures.rs"]
mod fixtures;

use fixtures::{apply_shared_rule, init_repo, sdk, sdk_error};
use serde_json::{json, Value};

fn neighbor_ids(answer: &Value, node_type: &str) -> Vec<String> {
    let mut ids = answer["neighbors"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|neighbor| neighbor["node"]["node_type"] == node_type)
        .map(|neighbor| neighbor["node"]["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

#[test]
fn neighbors_of_a_rule_are_the_requirements_that_produce_it() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);

    let answer = sdk(repo, "neighbors", &json!({"id": ids.rule.as_str()}));

    assert_eq!(
        answer["protocol_version"],
        provenance_core::SDK_PROTOCOL_VERSION
    );
    assert_eq!(answer["operation"], "neighbors");
    assert_eq!(answer["id"], ids.rule);
    assert_eq!(answer["has_more"], false);
    let mut expected = vec![ids.sharing.clone(), ids.sessions];
    expected.sort();
    assert_eq!(neighbor_ids(&answer, "requirement"), expected);
    assert!(answer["neighbors"]
        .as_array()
        .unwrap()
        .iter()
        .all(
            |neighbor| neighbor["relation"] == "requirement_ids" && neighbor["direction"] == "out"
        ));
}

#[test]
fn neighbors_of_a_requirement_are_its_rule_and_its_source() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);

    let answer = sdk(repo, "neighbors", &json!({"id": ids.sharing.as_str()}));

    assert_eq!(neighbor_ids(&answer, "rule"), vec![ids.rule]);
    assert_eq!(neighbor_ids(&answer, "source"), vec![ids.source]);
}

/// `out` reads the requirement's own fields: its citation names the
/// source. `in` reads the records whose fields name it: the rule's list.
#[test]
fn neighbors_reads_only_the_direction_the_caller_names() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);

    let outgoing = sdk(
        repo,
        "neighbors",
        &json!({"id": ids.sharing.as_str(), "direction": "out"}),
    );
    assert_eq!(neighbor_ids(&outgoing, "source"), vec![ids.source.clone()]);
    assert!(neighbor_ids(&outgoing, "rule").is_empty());

    let incoming = sdk(
        repo,
        "neighbors",
        &json!({"id": ids.sharing.as_str(), "direction": "in"}),
    );
    assert_eq!(neighbor_ids(&incoming, "rule"), vec![ids.rule]);
    assert!(neighbor_ids(&incoming, "source").is_empty());
}

#[test]
fn neighbors_refuses_a_relation_no_declaration_carries() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);

    let error = sdk_error(
        repo,
        "neighbors",
        &json!({"id": ids.rule.as_str(), "relations": ["produces"]}),
    );

    assert!(error.contains("unknown relation `produces`"), "{error}");
}

#[test]
fn neighbors_bounds_its_page_and_says_when_more_remain() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);

    let answer = sdk(repo, "neighbors", &json!({"id": ids.rule, "limit": 1}));

    assert_eq!(answer["neighbors"].as_array().unwrap().len(), 1);
    assert_eq!(answer["has_more"], true);
}
