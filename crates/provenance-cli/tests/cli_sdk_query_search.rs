#[path = "query_support/fixtures.rs"]
mod fixtures;

use fixtures::{apply_shared_rule, ids_of, init_repo, sdk, sdk_error};
use serde_json::json;

#[test]
fn search_matches_record_text_and_bounds_the_page() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);

    let answer = sdk(repo, "search", &json!({"text": "time bounded"}));

    assert_eq!(
        answer["protocol_version"],
        provenance_core::SDK_PROTOCOL_VERSION
    );
    assert_eq!(answer["operation"], "search");
    assert_eq!(answer["limit"], 50);
    assert_eq!(answer["has_more"], false);
    let mut found = ids_of(&answer["nodes"]);
    found.sort();
    let mut expected = vec![ids.sharing.clone(), ids.sessions];
    expected.sort();
    assert_eq!(found, expected);
    assert!(answer["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|node| node["node_type"] == "requirement"));
}

#[test]
fn search_says_when_more_records_match_than_the_limit_allows() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    apply_shared_rule(&directory);

    let answer = sdk(repo, "search", &json!({"text": "time bounded", "limit": 1}));

    assert_eq!(answer["limit"], 1);
    assert_eq!(answer["has_more"], true);
    assert_eq!(answer["nodes"].as_array().unwrap().len(), 1);
}

#[test]
fn search_reads_only_the_node_types_the_caller_names() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);

    let answer = sdk(
        repo,
        "search",
        &json!({"text": "expire", "node_types": ["rule"]}),
    );

    assert_eq!(ids_of(&answer["nodes"]), vec![ids.rule]);
}

#[test]
fn search_refuses_a_page_larger_than_the_engine_will_bound() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    apply_shared_rule(&directory);

    let error = sdk_error(repo, "search", &json!({"text": "time", "limit": 5000}));

    assert!(
        error.contains("limit must be between 1 and 200"),
        "error should name the cap: {error}"
    );
}
