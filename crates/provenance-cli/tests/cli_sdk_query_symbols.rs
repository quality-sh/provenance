#[path = "query_support/fixtures.rs"]
mod fixtures;

use fixtures::{apply_shared_rule, ids_of, init_repo, sdk, verify_rule};
use serde_json::json;

#[test]
fn resolve_symbol_names_the_rule_implemented_at_a_code_site() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);

    let answer = sdk(
        repo,
        "resolve-symbol",
        &json!({"file": "share-links.ts", "symbol": "createShareLink"}),
    );

    assert_eq!(
        answer["protocol_version"],
        provenance_core::SDK_PROTOCOL_VERSION
    );
    assert_eq!(answer["operation"], "resolve-symbol");
    assert_eq!(answer["file"], "share-links.ts");
    assert_eq!(answer["symbol"], "createShareLink");
    assert_eq!(answer["has_more"], false);
    assert_eq!(ids_of(&answer["rules"]), vec![ids.rule]);
    assert_eq!(answer["rules"][0]["node_type"], "rule");
}

#[test]
fn resolve_symbol_names_the_rule_verified_at_a_test_site() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);
    verify_rule(repo, &ids.rule);

    let answer = sdk(
        repo,
        "resolve-symbol",
        &json!({"file": "share-links.test.ts"}),
    );

    assert_eq!(ids_of(&answer["rules"]), vec![ids.rule]);
}

#[test]
fn resolve_symbol_reads_a_scanned_annotation_at_a_line() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let ids = apply_shared_rule(&directory);
    std::fs::write(
        directory.path().join("annotated.ts"),
        format!(
            "// @provenance rule: {}\nexport function later() {{}}\n",
            ids.rule
        ),
    )
    .unwrap();

    let answer = sdk(
        repo,
        "resolve-symbol",
        &json!({"file": "annotated.ts", "line": 1}),
    );

    assert_eq!(ids_of(&answer["rules"]), vec![ids.rule]);
}

#[test]
fn resolve_symbol_answers_nothing_for_a_file_no_rule_names() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    apply_shared_rule(&directory);

    let answer = sdk(repo, "resolve-symbol", &json!({"file": "unrelated.ts"}));

    assert_eq!(answer["rules"], json!([]));
    assert_eq!(answer["has_more"], false);
}
