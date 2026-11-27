//! An import in a repository whose state directory holds a link.
#![cfg(unix)]

use super::support::{export_scope, import_scope, init_repo, write_json};

/// An import with an error of its own, in a repository whose state
/// directory has a link, gives the error of the import: the check of the
/// state directory runs when the import writes. The Rule of the import
/// names a Requirement that the scope does not have.
#[test]
#[provenance_macros::verifies("rule_whole_set_refuses_a_link_in_the_state_directory", examples)]
fn an_import_error_comes_before_the_refusal_of_a_link_in_the_state() {
    use predicates::prelude::PredicateBooleanExt as _;
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init_repo(&repo, None);
    let external = dir.path().join("secret");
    std::fs::write(&external, "do not import").unwrap();
    std::os::unix::fs::symlink(&external, repo.join(".provenance/state/external-link")).unwrap();
    let baseline = dir.path().join("baseline.json");
    export_scope(&repo, &baseline).success();
    let mut invalid: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&baseline).unwrap()).unwrap();
    invalid["rules"] = serde_json::json!([{
        "scope_id": "default", "id": "rule_invalid",
        "statement": "A rule naming a requirement that is not there.", "status": "active",
        "severity": "high", "requirement_ids": ["req_missing"]
    }]);
    let input = dir.path().join("invalid.json");
    write_json(&input, &invalid);

    import_scope(&repo, &input)
        .failure()
        .stderr(predicates::str::contains("req_missing"))
        .stderr(predicates::str::contains("unsupported state entry").not());
}
