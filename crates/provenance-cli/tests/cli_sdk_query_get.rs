#[path = "query_support/fixtures.rs"]
mod fixtures;

use fixtures::{apply_shared_rule, init_repo, sdk, sdk_error};
use serde_json::json;

#[test]
fn get_returns_one_record_under_a_versioned_envelope() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);

    let answer = sdk(
        repo,
        "get",
        &json!({"node_type": "rule", "id": ids.rule.as_str()}),
    );

    assert_eq!(
        answer["protocol_version"],
        provenance_core::SDK_PROTOCOL_VERSION
    );
    assert_eq!(answer["operation"], "get");
    assert_eq!(answer["found"], true);
    assert_eq!(answer["node"]["node_type"], "rule");
    assert_eq!(answer["node"]["id"], ids.rule);
    assert_eq!(
        answer["node"]["statement"],
        "Share links expire within 30 days"
    );
}

#[test]
fn get_hides_a_retired_record_until_the_caller_asks_for_it() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);
    let mut narrowed = fixtures::shared_rule_spec();
    narrowed["requirements"] = json!([{
        "key": "sharing",
        "statement": "Shares are time bounded",
        "sources": ["retention"]
    }]);
    narrowed["rules"][0]["requirements"] = json!(["sharing"]);
    sdk(repo, "apply", &narrowed);

    let active = sdk(
        repo,
        "get",
        &json!({"node_type": "requirement", "id": ids.sessions.as_str()}),
    );
    assert_eq!(active["found"], false);
    assert!(active.get("node").is_none());

    let including = sdk(
        repo,
        "get",
        &json!({
            "node_type": "requirement",
            "id": ids.sessions.as_str(),
            "include_retired": true
        }),
    );
    assert_eq!(including["found"], true);
    assert_eq!(including["node"]["retired"], true);
    assert_eq!(including["node"]["id"], ids.sessions);
}

#[test]
fn get_refuses_a_request_written_for_another_protocol_version() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);

    let error = sdk_error(
        repo,
        "get",
        &json!({"protocol_version": 2, "node_type": "rule", "id": ids.rule}),
    );

    let spoken = format!("speaks {}", provenance_core::SDK_PROTOCOL_VERSION);
    assert!(
        error.contains("protocol version 2") && error.contains(&spoken),
        "error should name both versions: {error}"
    );
}
