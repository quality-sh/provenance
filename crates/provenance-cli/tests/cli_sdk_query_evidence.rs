#[path = "query_support/fixtures.rs"]
mod fixtures;

use fixtures::{apply_shared_rule, init_repo, restate_sharing, sdk, verify_rule};
use serde_json::json;

#[test]
fn evidence_separates_bindings_runs_review_and_stale_for_one_rule() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);
    verify_rule(repo, &ids.rule);

    let answer = sdk(repo, "evidence", &json!({"rule": ids.rule.as_str()}));

    assert_eq!(answer["protocol_version"], 6);
    assert_eq!(answer["operation"], "evidence");
    assert_eq!(answer["rule_id"], ids.rule);
    assert_eq!(
        answer["implementation_bindings"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        answer["implementation_bindings"][0]["symbol"],
        "createShareLink"
    );
    assert_eq!(answer["verification_bindings"].as_array().unwrap().len(), 1);
    assert_eq!(
        answer["verification_bindings"][0]["key"],
        "share-link-expiry"
    );
    assert_eq!(answer["verification_runs"].as_array().unwrap().len(), 1);
    assert_eq!(answer["latest_verification_run"]["status"], "passed");
    assert_eq!(answer["review_required"], false);
    assert_eq!(answer["reviews"], json!([]));
    assert_eq!(answer["stale"], json!(null));
}

#[test]
fn evidence_reports_review_required_after_the_requirement_is_restated() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);
    verify_rule(repo, &ids.rule);
    restate_sharing(repo);

    let answer = sdk(repo, "evidence", &json!({"rule": ids.rule.as_str()}));

    assert_eq!(answer["review_required"], true);
    assert_eq!(answer["reviews"].as_array().unwrap().len(), 1);
    assert_eq!(answer["reviews"][0]["requirement_id"], ids.sharing);
    assert_eq!(answer["reviews"][0]["field"], "statement");
    assert_eq!(answer["reviews"][0]["before"], "Shares are time bounded");
    assert_eq!(
        answer["reviews"][0]["after"],
        "Shares are time bounded and revocable"
    );
}

#[test]
fn a_verification_run_after_the_change_clears_the_review() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);
    verify_rule(repo, &ids.rule);
    restate_sharing(repo);
    verify_rule(repo, &ids.rule);

    let answer = sdk(repo, "evidence", &json!({"rule": ids.rule}));

    assert_eq!(answer["review_required"], false);
    assert_eq!(answer["verification_runs"].as_array().unwrap().len(), 2);
}

#[test]
fn evidence_leaves_out_retired_bindings_until_the_caller_asks_for_them() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);
    let mut without_implementation = fixtures::shared_rule_spec();
    without_implementation["rules"][0]
        .as_object_mut()
        .unwrap()
        .remove("implementation");
    sdk(repo, "apply", &without_implementation);

    let active = sdk(repo, "evidence", &json!({"rule": ids.rule.as_str()}));
    assert_eq!(active["implementation_bindings"], json!([]));

    let including = sdk(
        repo,
        "evidence",
        &json!({"rule": ids.rule, "include_retired": true}),
    );
    assert_eq!(
        including["implementation_bindings"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(including["implementation_bindings"][0]["retired"], true);
}
