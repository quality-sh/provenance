#[path = "query_support/fixtures.rs"]
mod fixtures;

use fixtures::{apply_shared_rule, init_repo, sdk, verify_rule};
use serde_json::json;

#[test]
fn impact_names_the_rules_a_requirement_reaches_with_their_sites() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);
    verify_rule(repo, &ids.rule);

    let answer = sdk(repo, "impact", &json!({"id": ids.sharing.as_str()}));

    assert_eq!(
        answer["protocol_version"],
        provenance_core::SDK_PROTOCOL_VERSION
    );
    assert_eq!(answer["operation"], "impact");
    assert_eq!(answer["id"], ids.sharing);
    assert_eq!(answer["has_more"], false);
    assert_eq!(answer["affected_rules"].as_array().unwrap().len(), 1);
    let affected = &answer["affected_rules"][0];
    assert_eq!(affected["id"], ids.rule);
    assert_eq!(
        affected["implementations"][0],
        json!({"file": "share-links.ts", "symbol": "createShareLink"})
    );
    assert_eq!(
        affected["verifications"][0],
        json!({
            "key": "share-link-expiry",
            "method": "examples",
            "declared_by": "ci://typescript",
            "file": "share-links.test.ts",
            "symbol": "expiry test"
        })
    );
}

#[test]
fn impact_of_a_source_reaches_the_rules_behind_both_requirements() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);

    let answer = sdk(repo, "impact", &json!({"id": ids.source}));

    let reached = answer["affected_rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|rule| rule["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(reached, vec![ids.rule]);
}

#[test]
fn impact_bounds_its_page_and_says_when_more_remain() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);
    let mut widened = fixtures::shared_rule_spec();
    widened["rules"] = json!([
        {
            "key": "expiry",
            "requirements": ["sharing", "sessions"],
            "statement": "Share links expire within 30 days"
        },
        {
            "key": "revocation",
            "requirements": ["sharing"],
            "statement": "Share links must be revocable"
        }
    ]);
    sdk(repo, "apply", &widened);

    let answer = sdk(repo, "impact", &json!({"id": ids.sharing, "limit": 1}));

    assert_eq!(answer["affected_rules"].as_array().unwrap().len(), 1);
    assert_eq!(answer["has_more"], true);
}
