#[path = "query_support/fixtures.rs"]
mod fixtures;

use fixtures::{apply_shared_rule, init_repo, sdk, sdk_raw, verify_rule};
use serde_json::{json, Value};

fn twice(repo: &str, command: &str, request: &Value) -> String {
    let (first_success, first, _) = sdk_raw(repo, command, request);
    let (second_success, second, _) = sdk_raw(repo, command, request);
    assert!(first_success && second_success, "sdk {command} failed");
    assert_eq!(
        first, second,
        "sdk {command} answered differently on a second run"
    );
    first
}

#[test]
fn an_agent_answers_impact_evidence_neighbors_and_trace_from_bounded_responses() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);
    verify_rule(repo, &ids.rule);

    let impact = sdk(repo, "impact", &json!({"id": ids.sharing.as_str()}));
    assert_eq!(impact["affected_rules"][0]["id"], ids.rule);
    assert_eq!(
        impact["affected_rules"][0]["implementations"][0]["symbol"],
        "createShareLink"
    );
    assert_eq!(
        impact["affected_rules"][0]["verifications"][0]["key"],
        "share-link-expiry"
    );

    let evidence = sdk(repo, "evidence", &json!({"rule": ids.rule.as_str()}));
    assert_eq!(
        evidence["implementation_bindings"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        evidence["verification_bindings"].as_array().unwrap().len(),
        1
    );
    assert_eq!(evidence["latest_verification_run"]["status"], "passed");
    assert_eq!(evidence["review_required"], false);
    assert_eq!(evidence["stale"], json!(null));

    let neighbours = sdk(repo, "neighbors", &json!({"id": ids.rule.as_str()}));
    let mut requirements = neighbours["neighbors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|neighbor| neighbor["node"]["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    requirements.sort();
    let mut expected = vec![ids.sharing.clone(), ids.sessions.clone()];
    expected.sort();
    assert_eq!(requirements, expected);

    // The Source sits one hop further out, on the Requirement.
    let from_requirement = sdk(repo, "neighbors", &json!({"id": ids.sharing.as_str()}));
    assert!(from_requirement["neighbors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|neighbor| neighbor["node"]["id"] == ids.source.as_str()));

    let trace = sdk(
        repo,
        "trace",
        &json!({"id": ids.source.as_str(), "direction": "in"}),
    );
    assert!(trace["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reached| reached["node"]["id"] == ids.rule.as_str() && reached["depth"] == 2));
}

#[test]
fn every_primitive_answers_the_same_bytes_on_a_second_run() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);
    verify_rule(repo, &ids.rule);

    for (command, request) in [
        ("get", json!({"node_type": "rule", "id": ids.rule.as_str()})),
        ("search", json!({"text": "time bounded"})),
        ("neighbors", json!({"id": ids.rule.as_str()})),
        (
            "trace",
            json!({"id": ids.source.as_str(), "direction": "out"}),
        ),
        ("impact", json!({"id": ids.sharing.as_str()})),
        ("evidence", json!({"rule": ids.rule.as_str()})),
        (
            "resolve-symbol",
            json!({"file": "share-links.ts", "symbol": "createShareLink"}),
        ),
    ] {
        let answer = twice(repo, command, &request);
        let parsed: Value = serde_json::from_str(&answer).unwrap();
        assert_eq!(
            parsed["protocol_version"],
            provenance_core::SDK_PROTOCOL_VERSION,
            "{command} names the protocol"
        );
        assert!(
            parsed.get("operation").is_some(),
            "{command} names its operation"
        );
    }
}
