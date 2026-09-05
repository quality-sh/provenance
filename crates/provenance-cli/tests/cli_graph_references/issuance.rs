use super::*;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_macros::verifies;

#[test]
fn issue_is_versioned_deterministic_and_correlation_is_not_identity() {
    let temp = committed_store();
    let first = issue(temp.path(), &[]);
    let second = issue(
        temp.path(),
        &[
            "--correlation-system",
            "github",
            "--correlation-key",
            "owner/repo#42",
        ],
    );

    assert_eq!(first["schema_version"], SUPPORTED_SCHEMA_VERSION.0);
    assert_eq!(first["reference_id"], second["reference_id"]);
    assert_eq!(first["graph_digest"], second["graph_digest"]);
    assert_eq!(second["correlation"]["system"], "github");
    assert_eq!(second["correlation"]["key"], "owner/repo#42");
    assert_eq!(first["commit"].as_str().unwrap().len(), 40);
}

#[test]
fn implicit_head_requires_only_relevant_canonical_state_to_be_clean() {
    let temp = committed_store();
    std::fs::write(temp.path().join("unrelated.txt"), "dirty but irrelevant\n").unwrap();
    issue(temp.path(), &[]);

    let manifest = temp.path().join(".provenance/state/manifest.json");
    let original = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(&manifest, original.replace("\".\"", "\"src\"")).unwrap();
    provenance(temp.path())
        .args([
            "graph-reference",
            "issue",
            "--repo",
            ".",
            "--scope",
            "default",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("incomplete"));

    std::fs::write(&manifest, original.replace("\".\"", "\"staged\"")).unwrap();
    git(temp.path(), &["add", ".provenance/state/manifest.json"]);
    std::fs::write(&manifest, &original).unwrap();
    provenance(temp.path())
        .args([
            "graph-reference",
            "issue",
            "--repo",
            ".",
            "--scope",
            "default",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("incomplete"));
}

#[test]
fn explicit_commit_allows_dirty_state_and_exact_operations_read_the_pin() {
    let temp = committed_store();
    let reference = issue(temp.path(), &[]);
    let reference_path = write_reference(temp.path(), &reference);
    let manifest = temp.path().join(".provenance/state/manifest.json");
    std::fs::write(&manifest, "not the pinned manifest\n").unwrap();

    for operation in ["show", "verify", "exact-export"] {
        provenance(temp.path())
            .args([
                "graph-reference",
                operation,
                "--repo",
                ".",
                "--reference",
                reference_path.to_str().unwrap(),
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains(format!(
                "\"schema_version\": {}",
                SUPPORTED_SCHEMA_VERSION.0
            )));
    }
}

/// The refusals reach the operator. The four checks are exercised arm by arm
/// against the typed error in
/// `provenance-store/tests/graph_reference_verification.rs`; what is at stake
/// here is that a failed check leaves the command with a non-zero exit and a
/// message naming the field, rather than printing a graph nobody verified.
#[test]
#[verifies("rule_reference_verified", examples)]
fn refused_references_fail_every_read_verb_and_name_the_field() {
    let temp = committed_store();
    let genuine = issue(temp.path(), &[]);

    let mut tampered = genuine.clone();
    tampered["graph_digest"] = Value::String(format!("sha256:{}", "0".repeat(64)));
    let reference_path = write_reference(temp.path(), &tampered);
    for operation in ["show", "verify", "exact-export"] {
        provenance(temp.path())
            .args([
                "graph-reference",
                operation,
                "--repo",
                ".",
                "--reference",
                reference_path.to_str().unwrap(),
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "mismatched graph reference graph_digest",
            ));
    }

    let mut tampered = genuine.clone();
    tampered["reference_id"] = Value::String(format!("grf1_{}", "0".repeat(64)));
    let reference_path = write_reference(temp.path(), &tampered);
    provenance(temp.path())
        .args([
            "graph-reference",
            "verify",
            "--repo",
            ".",
            "--reference",
            reference_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "mismatched graph reference reference_id",
        ));

    let mut tampered = genuine;
    tampered["commit"] = Value::String("0".repeat(40));
    let reference_path = write_reference(temp.path(), &tampered);
    provenance(temp.path())
        .args([
            "graph-reference",
            "show",
            "--repo",
            ".",
            "--reference",
            reference_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing").and(predicate::str::contains("0".repeat(40))));
}

#[test]
fn incomplete_reference_and_workflow_specific_fields_are_rejected() {
    let temp = committed_store();
    let mut reference = issue(temp.path(), &[]);
    reference["workflow_id"] = Value::String("workflowd-123".into());
    let reference_path = write_reference(temp.path(), &reference);
    provenance(temp.path())
        .args([
            "graph-reference",
            "verify",
            "--repo",
            ".",
            "--reference",
            reference_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("incomplete"));

    let mut reference = issue(temp.path(), &[]);
    reference.as_object_mut().unwrap().remove("graph_digest");
    let reference_path = write_reference(temp.path(), &reference);
    provenance(temp.path())
        .args([
            "graph-reference",
            "verify",
            "--repo",
            ".",
            "--reference",
            reference_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("incomplete"));
}

#[test]
fn identity_changes_by_commit_and_graph_while_digest_tracks_graph_only() {
    let temp = committed_store();
    let empty = issue(temp.path(), &[]);
    git(
        temp.path(),
        &["commit", "--allow-empty", "-qm", "new handoff commit"],
    );
    let same_graph = issue(temp.path(), &[]);
    assert_eq!(empty["graph_digest"], same_graph["graph_digest"]);
    assert_ne!(empty["reference_id"], same_graph["reference_id"]);

    provenance(temp.path())
        .args([
            "requirements",
            "create",
            "--repo",
            ".",
            "--scope",
            "default",
            "--id",
            "req_pinned",
            "--statement",
            "Pinned graph content",
        ])
        .assert()
        .success();
    git(temp.path(), &["add", ".provenance/state"]);
    git(temp.path(), &["commit", "-qm", "change canonical graph"]);
    let changed_graph = issue(temp.path(), &[]);
    assert_ne!(same_graph["graph_digest"], changed_graph["graph_digest"]);
    assert_ne!(same_graph["reference_id"], changed_graph["reference_id"]);

    let old_reference = write_reference(temp.path(), &same_graph);
    let output = provenance(temp.path())
        .args([
            "graph-reference",
            "exact-export",
            "--repo",
            ".",
            "--reference",
            old_reference.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let export: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(export["graph"]["requirements"], serde_json::json!([]));
}
