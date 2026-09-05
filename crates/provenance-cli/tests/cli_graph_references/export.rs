use super::*;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_macros::verifies;
use provenance_store::graph_reference::{graph_digest, GraphExport};
use serde_json::json;

/// The exported document, read back away from the repository that produced it.
///
/// The command is run from an unrelated directory with no `.provenance` state
/// and no Git history, so anything it can say about the document it has to get
/// from the document.
fn validate_elsewhere(document: &Value) -> assert_cmd::assert::Assert {
    let elsewhere = tempfile::tempdir().unwrap();
    let path = elsewhere.path().join("export.json");
    std::fs::write(&path, serde_json::to_vec(document).unwrap()).unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(elsewhere.path())
        .args([
            "validate",
            "graph-reference-export",
            "--input",
            path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
}

/// The document a reference hands out carries the digest the reference names,
/// and is checkable on its own: a record edited inside it is refused by a
/// holder with no repository, in an error naming the digest claimed and the
/// digest the graph in front of them hashes to.
#[test]
fn exported_document_verifies_itself_away_from_its_repository() {
    let temp = committed_store();
    provenance(temp.path())
        .args([
            "sources",
            "create",
            "--repo",
            ".",
            "--scope",
            "default",
            "--id",
            "source_policy",
            "--name",
            "Retention policy",
        ])
        .assert()
        .success();
    git(temp.path(), &["add", ".provenance/state"]);
    git(temp.path(), &["commit", "-qm", "add pinned source"]);
    let reference = issue(temp.path(), &[]);
    let reference_path = write_reference(temp.path(), &reference);
    let output = provenance(temp.path())
        .args([
            "graph-reference",
            "exact-export",
            "--repo",
            ".",
            "--reference",
            reference_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let mut document: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(document["graph_digest"], reference["graph_digest"]);
    validate_elsewhere(&document).success();

    document["graph"]["sources"][0]["name"] = json!("Retention policy (superseded)");
    let edited: GraphExport = serde_json::from_value(document["graph"].clone()).unwrap();
    let edited = graph_digest(&edited).unwrap();
    validate_elsewhere(&document)
        .failure()
        .stderr(predicate::str::contains(
            reference["graph_digest"].as_str().unwrap(),
        ))
        .stderr(predicate::str::contains(edited));
}

#[test]
fn explicit_commit_issues_from_pin_despite_relevant_staged_and_worktree_changes() {
    let temp = committed_store();
    let head = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    let head = String::from_utf8(head.stdout).unwrap().trim().to_string();
    let manifest = temp.path().join(".provenance/state/manifest.json");
    let original = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(&manifest, original.replace("\".\"", "\"staged\"")).unwrap();
    git(temp.path(), &["add", ".provenance/state/manifest.json"]);
    std::fs::write(&manifest, original.replace("\".\"", "\"worktree\"")).unwrap();

    let reference = issue(temp.path(), &["--commit", &head]);
    assert_eq!(reference["commit"], head);
}

#[test]
#[verifies("rule_pinned_graph_families", construction)]
fn exact_export_contains_only_canonical_graph_families() {
    let temp = committed_store();
    let proposal_dir = temp
        .path()
        .join(".provenance/state/scopes/default/ideation");
    std::fs::create_dir_all(&proposal_dir).unwrap();
    std::fs::write(
        proposal_dir.join("proposal_cards.jsonl"),
        concat!(
            "{\"schema_version\":1,\"scope_id\":\"default\",",
            "\"id\":\"proposal_workflowd_123\",\"proposal_key\":\"workflowd-123\",",
            "\"proposal_type\":\"no_action\",\"title\":\"No graph change\",",
            "\"summary\":\"Collaboration-only proposal\",\"traceability\":{",
            "\"target\":{\"artifact_type\":\"requirement\",\"artifact_id\":\"req_none\"},",
            "\"source_ids\":[],\"evidence_references\":[],\"supporting_claim_ids\":[]},",
            "\"promotion_state\":\"proposed\"}\n"
        ),
    )
    .unwrap();
    git(temp.path(), &["add", ".provenance/state"]);
    git(
        temp.path(),
        &["commit", "-qm", "non-graph collaboration state"],
    );
    let reference = issue(temp.path(), &[]);
    let reference_path = write_reference(temp.path(), &reference);

    let output = provenance(temp.path())
        .args([
            "graph-reference",
            "exact-export",
            "--repo",
            ".",
            "--reference",
            reference_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let export: Value = serde_json::from_slice(&output).unwrap();
    let graph = export["graph"].as_object().unwrap();
    assert!(!graph.contains_key("proposal_cards"));
    assert!(!graph.contains_key("promotion_decisions"));
    assert!(!String::from_utf8(output.clone())
        .unwrap()
        .contains("proposal_workflowd_123"));
    assert!(!String::from_utf8(output).unwrap().contains("workflowd-123"));
}

#[test]
#[verifies("rule_export_strips_collaboration", examples)]
fn collaboration_claims_do_not_change_digest_or_appear_in_exact_export() {
    let temp = committed_store();
    provenance(temp.path())
        .args([
            "sources",
            "create",
            "--repo",
            ".",
            "--scope",
            "default",
            "--id",
            "source_origin",
            "--name",
            "Origin metadata source",
            "--origin-thread",
            "thread_private",
            "--origin-message",
            "message_private",
        ])
        .assert()
        .success();
    provenance(temp.path())
        .args([
            "requirements",
            "create",
            "--repo",
            ".",
            "--scope",
            "default",
            "--id",
            "req_claims",
            "--statement",
            "Claims are collaboration metadata",
        ])
        .assert()
        .success();
    provenance(temp.path())
        .args([
            "topics",
            "create",
            "--repo",
            ".",
            "--scope",
            "default",
            "--id",
            "topic_claims",
            "--requirement-id",
            "req_claims",
            "--title",
            "Claim handling",
        ])
        .assert()
        .success();
    git(temp.path(), &["add", ".provenance/state"]);
    git(temp.path(), &["commit", "-qm", "add graph topic"]);
    let unclaimed = issue(temp.path(), &[]);

    provenance(temp.path())
        .args([
            "topics",
            "claim",
            "--repo",
            ".",
            "--scope",
            "default",
            "--id",
            "topic_claims",
            "--actor",
            "workflowd-123",
        ])
        .assert()
        .success();
    git(temp.path(), &["add", ".provenance/state"]);
    git(temp.path(), &["commit", "-qm", "claim graph topic"]);
    let claimed = issue(temp.path(), &[]);

    assert_eq!(unclaimed["graph_digest"], claimed["graph_digest"]);
    let reference_path = write_reference(temp.path(), &claimed);
    let output = provenance(temp.path())
        .args([
            "graph-reference",
            "exact-export",
            "--repo",
            ".",
            "--reference",
            reference_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let output = String::from_utf8(output).unwrap();
    assert!(!output.contains("claimed_by"));
    assert!(!output.contains("claimed_at"));
    assert!(!output.contains("workflowd-123"));
    assert!(!output.contains("origin_thread"));
    assert!(!output.contains("origin_message"));
    assert!(!output.contains("thread_private"));
    assert!(!output.contains("message_private"));
}

#[test]
fn implementation_binding_travels_in_the_exact_graph_and_changes_its_digest() {
    let temp = committed_store();
    provenance(temp.path())
        .args([
            "requirements",
            "create",
            "--repo",
            ".",
            "--scope",
            "default",
            "--id",
            "req_runtime",
            "--statement",
            "Accepted workflows start",
        ])
        .assert()
        .success();
    provenance(temp.path())
        .args([
            "rules",
            "create",
            "--repo",
            ".",
            "--scope",
            "default",
            "--id",
            "rule_runtime",
            "--requirement-id",
            "req_runtime",
            "--statement",
            "Accepted workflows start",
        ])
        .assert()
        .success();
    git(temp.path(), &["add", ".provenance/state"]);
    git(temp.path(), &["commit", "-qm", "add runtime rule"]);
    let without_binding = issue(temp.path(), &[]);

    let path = temp
        .path()
        .join(".provenance/state/scopes/default/implementations/binding.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        format!(
            concat!(
                "{{\"schema_version\":{},\"scope_id\":\"default\",",
                "\"id\":\"implementation_binding_runtime\",\"rule_id\":\"rule_runtime\",",
                "\"declared_by\":\"spec://typescript/workflows\",",
                "\"file\":\"src/runtime.ts\",\"symbol\":\"startWorkflow\"}}\n"
            ),
            SUPPORTED_SCHEMA_VERSION.0
        ),
    )
    .unwrap();
    git(temp.path(), &["add", ".provenance/state"]);
    git(
        temp.path(),
        &["commit", "-qm", "bind runtime implementation"],
    );
    let with_binding = issue(temp.path(), &[]);
    assert_ne!(
        without_binding["graph_digest"],
        with_binding["graph_digest"]
    );

    let reference_path = write_reference(temp.path(), &with_binding);
    let output = provenance(temp.path())
        .args([
            "graph-reference",
            "exact-export",
            "--repo",
            ".",
            "--reference",
            reference_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let document: Value = serde_json::from_slice(&output).unwrap();
    let bindings = document["graph"]["implementation_bindings"]
        .as_array()
        .unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0]["rule_id"], "rule_runtime");
    validate_elsewhere(&document).success();
}
