//! The kernel-equivalence suite (V2).
//!
//! Identical raw declarations through the kernel in process, through the
//! wire with frontend-computed extras, and through the minimal wire all
//! yield the same store outcome. Wire ingestion keeps decoded order and
//! today's first error for a multi-defect document.

use assert_cmd::Command;
use provenance_core::authoring::{requirement, rule, source, spec};
use provenance_core::ScopeId;
use provenance_macros::verifies;
use provenance_store::operations;
use serde_json::{json, Value};

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn init_repo() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    provenance()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    directory
}

fn repo_root(directory: &tempfile::TempDir) -> camino::Utf8PathBuf {
    camino::Utf8PathBuf::from_path_buf(directory.path().canonicalize().unwrap()).unwrap()
}

fn wire_apply(repo: &str, input: &Value) -> (bool, Value, String) {
    let output = provenance()
        .args([
            "sdk", "apply", "--repo", repo, "--scope", "default", "--format", "json",
        ])
        .write_stdin(serde_json::to_vec(input).unwrap())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let value = if output.status.success() {
        serde_json::from_slice(&output.stdout).unwrap()
    } else {
        Value::Null
    };
    (output.status.success(), value, stderr)
}

fn kernel_document() -> provenance_core::authoring::SpecDocument {
    spec("share-links")
        .requirements([
            requirement("sharing")
                .statement("Users can securely share documentation")
                .from(source("sharing-policy").document("docs/sharing-policy.md"))
                .rules([
                    rule("expiry").statement("Share links expire within 30 days"),
                    rule("audit")
                        .statement("Share-link access is audited")
                        .requirements(["retention"]),
                ]),
            requirement("retention").statement("Share records are retained"),
        ])
        .build()
        .unwrap()
}

#[test]
#[verifies("rule_rust_wire_acceptance_is_stable", conformance)]
fn kernel_direct_and_both_wire_shapes_yield_one_outcome() {
    let canonical = kernel_document().materialize("spec://parity");
    let wire_full: Value = serde_json::to_value(&canonical).unwrap();
    let mut wire_minimal = wire_full.clone();
    for rule in wire_minimal["rules"].as_array_mut().unwrap() {
        rule.as_object_mut().unwrap().remove("address");
    }

    let in_process = init_repo();
    let direct = operations::apply(
        Some(repo_root(&in_process)),
        &ScopeId::new("default").unwrap(),
        canonical,
    )
    .unwrap();
    let direct: Value = serde_json::to_value(&direct).unwrap();

    let full_repo = init_repo();
    let (accepted, full, stderr) = wire_apply(full_repo.path().to_str().unwrap(), &wire_full);
    assert!(accepted, "wire-with-extras rejected: {stderr}");

    let minimal_repo = init_repo();
    let (accepted, minimal, stderr) =
        wire_apply(minimal_repo.path().to_str().unwrap(), &wire_minimal);
    assert!(accepted, "wire-minimal rejected: {stderr}");

    assert_eq!(direct, full);
    assert_eq!(direct, minimal);
    assert_eq!(direct["created"], json!(5));
}

#[test]
#[verifies("rule_rust_wire_order_is_preserved", examples)]
fn wire_ingestion_returns_resources_in_decoded_order() {
    let repo = init_repo();
    let input = json!({
        "schema_version": 1,
        "spec": "keys",
        "declared_by": "spec://parity",
        "requirements": [
            {"key": "b-lower", "statement": "Statement one"},
            {"key": "A-upper", "statement": "Statement two"},
            {"key": "ä-umlaut", "statement": "Statement three"}
        ]
    });

    let (accepted, result, stderr) = wire_apply(repo.path().to_str().unwrap(), &input);

    assert!(accepted, "apply rejected: {stderr}");
    let keys = result["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|resource| resource["key"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(keys, ["b-lower", "A-upper", "ä-umlaut"]);
}

#[test]
#[verifies("rule_rust_wire_first_error_is_stable", conformance)]
fn a_multi_defect_document_keeps_its_first_error_on_both_routes() {
    let input = json!({
        "schema_version": 1,
        "spec": "share-links",
        "declared_by": "spec://parity",
        "sources": [
            {"key": "policy", "name": "Policy", "kind": "document", "reference": "docs/a.md"},
            {"key": "policy", "name": "Policy twin", "kind": "document", "reference": "docs/b.md"}
        ],
        "requirements": [
            {"key": "sharing", "statement": "Users can share documentation", "sources": ["absent"]}
        ],
        "rules": [
            {"key": "", "requirement": "sharing", "statement": "Broken"}
        ]
    });

    let wire_repo = init_repo();
    let (accepted, _, stderr) = wire_apply(wire_repo.path().to_str().unwrap(), &input);
    assert!(!accepted);
    assert!(
        stderr.contains("duplicate source key `policy`"),
        "wire route reported another first error: {stderr}"
    );

    let direct_repo = init_repo();
    let error = operations::apply(
        Some(repo_root(&direct_repo)),
        &ScopeId::new("default").unwrap(),
        serde_json::from_value(input).unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "duplicate source key `policy`");
}

#[test]
fn wire_ingestion_rejects_the_named_structural_defects() {
    let repo = init_repo();
    let repo = repo.path().to_str().unwrap();
    let base = |requirements: Value, rules: Value| {
        json!({
            "schema_version": 1,
            "spec": "share-links",
            "declared_by": "spec://parity",
            "requirements": requirements,
            "rules": rules
        })
    };

    let cases = [
        (
            base(
                json!([
                    {"key": "sharing", "statement": "One"},
                    {"key": "sharing", "statement": "Two"}
                ]),
                json!([]),
            ),
            "duplicate requirement key `sharing`",
        ),
        (
            base(
                json!([{"key": "sharing", "statement": "One"}]),
                json!([{"key": "expiry", "requirement": "absent", "statement": "Broken"}]),
            ),
            "references undeclared requirement `absent`",
        ),
        (
            base(
                json!([{"key": "sharing", "statement": "One"}]),
                json!([{
                    "key": "expiry",
                    "requirement": "sharing",
                    "requirements": ["sharing"],
                    "statement": "Broken"
                }]),
            ),
            "cannot set both `requirement` and `requirements`",
        ),
        (
            base(
                json!([{"key": "sharing", "statement": "One"}]),
                json!([{
                    "key": "expiry",
                    "requirements": ["sharing", "sharing"],
                    "statement": "Broken"
                }]),
            ),
            "repeats requirement `sharing`",
        ),
    ];
    for (input, expected) in cases {
        let (accepted, _, stderr) = wire_apply(repo, &input);
        assert!(!accepted, "document was accepted: {input}");
        assert!(
            stderr.contains(expected),
            "missing `{expected}` in: {stderr}"
        );
    }

    let unknown_field = json!({
        "schema_version": 1,
        "spec": "share-links",
        "declared_by": "spec://parity",
        "surprise": true
    });
    let (accepted, _, stderr) = wire_apply(repo, &unknown_field);
    assert!(!accepted);
    assert!(stderr.contains("unknown field `surprise`"), "{stderr}");
}

#[test]
fn a_legacy_singular_requirement_field_matches_the_normalized_list() {
    let legacy_repo = init_repo();
    let list_repo = init_repo();
    let document = |rule: Value| {
        json!({
            "schema_version": 1,
            "spec": "share-links",
            "declared_by": "spec://parity",
            "requirements": [{"key": "sharing", "statement": "Users can share documentation"}],
            "rules": [rule]
        })
    };

    let (accepted, legacy, stderr) = wire_apply(
        legacy_repo.path().to_str().unwrap(),
        &document(
            json!({"key": "expiry", "requirement": "sharing", "statement": "Share links expire"}),
        ),
    );
    assert!(accepted, "{stderr}");
    let (accepted, list, stderr) = wire_apply(
        list_repo.path().to_str().unwrap(),
        &document(
            json!({"key": "expiry", "requirements": ["sharing"], "statement": "Share links expire"}),
        ),
    );
    assert!(accepted, "{stderr}");

    assert_eq!(legacy["resources"], list["resources"]);
}
