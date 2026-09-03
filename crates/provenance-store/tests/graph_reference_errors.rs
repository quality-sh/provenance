use provenance_core::{Manifest, RepoPathPrefix, ScopeId};
use provenance_macros::verifies;
use provenance_store::{
    graph_reference::{
        graph_digest, ExactExport, ExternalCorrelation, GraphExport, GraphReference,
        GraphReferenceError, GraphReferences,
    },
    layout::ProvenanceLayout,
};
use serde_json::json;

fn git(repo: &std::path::Path, args: &[&str]) {
    assert!(
        std::process::Command::new("git")
            .args(args)
            .current_dir(repo)
            .status()
            .unwrap()
            .success(),
        "git {args:?} failed"
    );
}

fn committed_store() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    git(temp.path(), &["init", "-q"]);
    git(temp.path(), &["add", ".provenance/state"]);
    git(
        temp.path(),
        &[
            "-c",
            "user.name=Graph Reference Test",
            "-c",
            "user.email=graph-reference@example.invalid",
            "commit",
            "-qm",
            "canonical graph",
        ],
    );
    temp
}

/// A reference document with every field well formed, before one field is
/// replaced with the value under test.
fn reference_document(field: &str, value: serde_json::Value) -> Vec<u8> {
    let mut document = json!({
        "schema_version": 1,
        "reference_id": format!("grf1_{}", "0".repeat(64)),
        "repository_id": format!("git1_{}", "1".repeat(64)),
        "store_path": ".provenance/state",
        "scope_id": "default",
        "commit": "2".repeat(40),
        "graph_digest": format!("sha256:{}", "3".repeat(64))
    });
    document[field] = value;
    serde_json::to_vec(&document).unwrap()
}

/// Reads a refusal back: the error is always Incomplete, and its detail is
/// returned so each test can check the reference names the field it refused.
fn refusal(field: &str, value: serde_json::Value) -> String {
    match GraphReference::from_json(&reference_document(field, value)) {
        Err(GraphReferenceError::Incomplete { detail }) => detail,
        Err(other) => panic!("{field} refusal should be Incomplete, got {other:?}"),
        Ok(reference) => panic!("{field} should have been refused, got {reference:?}"),
    }
}

#[test]
#[verifies("rule_reference_wellformed", examples)]
fn well_formed_reference_is_accepted() {
    let reference = GraphReference::from_json(&reference_document("scope_id", json!("default")))
        .expect("a well-formed reference document is accepted");

    assert_eq!(reference.schema_version, 1);
    assert_eq!(reference.store_path, ".provenance/state");
    assert_eq!(reference.correlation, None);
}

#[test]
#[verifies("rule_reference_wellformed", examples)]
fn reference_id_must_be_grf1_and_64_lowercase_hex_digits() {
    for value in [
        format!("git1_{}", "0".repeat(64)),
        "0".repeat(64),
        format!("grf1_{}", "0".repeat(63)),
        format!("grf1_{}", "0".repeat(65)),
        format!("grf1_{}", "A".repeat(64)),
    ] {
        assert!(
            refusal("reference_id", json!(value)).contains("reference_id"),
            "refusing reference_id '{value}' should name the field"
        );
    }
}

#[test]
#[verifies("rule_reference_wellformed", examples)]
fn repository_id_must_be_git1_and_64_lowercase_hex_digits() {
    for value in [
        format!("grf1_{}", "0".repeat(64)),
        "0".repeat(64),
        format!("git1_{}", "0".repeat(63)),
        format!("git1_{}", "0".repeat(65)),
        format!("git1_{}", "A".repeat(64)),
    ] {
        assert!(
            refusal("repository_id", json!(value)).contains("repository_id"),
            "refusing repository_id '{value}' should name the field"
        );
    }
}

#[test]
#[verifies("rule_reference_wellformed", examples)]
fn graph_digest_must_be_a_sha256_digest() {
    for value in [
        "0".repeat(64),
        format!("sha1:{}", "0".repeat(64)),
        format!("SHA256:{}", "0".repeat(64)),
        format!("sha256:{}", "0".repeat(63)),
        format!("sha256:{}", "0".repeat(65)),
        format!("sha256:{}", "A".repeat(64)),
    ] {
        assert!(
            refusal("graph_digest", json!(value)).contains("graph_digest"),
            "refusing graph_digest '{value}' should name the field"
        );
    }
}

#[test]
#[verifies("rule_reference_wellformed", examples)]
fn store_path_is_fixed() {
    for value in ["", "provenance/state", ".provenance/state/", ".provenance"] {
        assert!(
            refusal("store_path", json!(value)).contains(".provenance/state"),
            "refusing store_path '{value}' should state the one path allowed"
        );
    }
}

#[test]
#[verifies("rule_reference_wellformed", examples)]
fn scope_id_must_be_a_scope_name() {
    for value in ["", "Default", "de fault", "default/child"] {
        assert!(
            refusal("scope_id", json!(value)).contains("scope id"),
            "refusing scope_id '{value}' should say what a scope id may contain"
        );
    }
}

#[test]
#[verifies("rule_reference_wellformed", examples)]
fn commit_must_be_a_full_lowercase_object_id() {
    for value in [
        "0".repeat(39),
        "0".repeat(41),
        "0".repeat(63),
        "0".repeat(65),
        "A".repeat(40),
        "g".repeat(40),
        String::new(),
    ] {
        assert!(
            refusal("commit", json!(value)).contains("40- or 64-character"),
            "refusing commit '{value}' should state the lengths allowed"
        );
    }
    for value in ["4".repeat(40), "4".repeat(64)] {
        GraphReference::from_json(&reference_document("commit", json!(value)))
            .expect("a full 40- or 64-character lowercase commit id is accepted");
    }
}

#[test]
#[verifies("rule_reference_wellformed", examples)]
fn correlation_must_be_whole_when_present() {
    for value in [
        json!(null),
        json!({}),
        json!({"system": "github"}),
        json!({"key": "issue-35"}),
        json!({"system": "", "key": "issue-35"}),
        json!({"system": "   ", "key": "issue-35"}),
        json!({"system": "github", "key": "  "}),
        json!({"system": "github", "key": "issue-35", "note": "extra"}),
    ] {
        let detail = refusal("correlation", value.clone());
        assert!(
            !detail.is_empty(),
            "refusing correlation {value} said nothing"
        );
    }
    GraphReference::from_json(&reference_document(
        "correlation",
        json!({"system": "github", "key": "issue-35"}),
    ))
    .expect("a correlation with both parts filled is accepted");
}

#[test]
fn malformed_reference_is_typed_as_incomplete() {
    let error = GraphReference::from_json(br#"{"schema_version":1}"#).unwrap_err();
    assert!(matches!(error, GraphReferenceError::Incomplete { .. }));
}

#[test]
fn unsupported_reference_version_is_typed_as_incomplete() {
    let document = br#"{
        "schema_version": 2,
        "reference_id": "grf1_x",
        "repository_id": "git1_x",
        "store_path": ".provenance/state",
        "scope_id": "default",
        "commit": "0000000000000000000000000000000000000000",
        "graph_digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000"
    }"#;
    let error = GraphReference::from_json(document).unwrap_err();
    assert!(matches!(error, GraphReferenceError::Incomplete { .. }));
}

#[test]
fn store_api_rejects_invalid_reference_contract() {
    let temp = committed_store();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let references = GraphReferences::open(root).unwrap();
    let reference = references.issue("default", None, None).unwrap();

    let mut unsupported = reference.clone();
    unsupported.schema_version = 2;
    assert!(matches!(
        references.show(&unsupported),
        Err(GraphReferenceError::Incomplete { .. })
    ));
    assert!(matches!(
        references.verify(&unsupported),
        Err(GraphReferenceError::Incomplete { .. })
    ));
    assert!(matches!(
        references.exact_export(&unsupported),
        Err(GraphReferenceError::Incomplete { .. })
    ));

    let mut invalid_correlation = reference;
    invalid_correlation.correlation = Some(ExternalCorrelation {
        system: " ".into(),
        key: "issue-35".into(),
    });
    assert!(matches!(
        references.show(&invalid_correlation),
        Err(GraphReferenceError::Incomplete { .. })
    ));
    assert!(matches!(
        references.verify(&invalid_correlation),
        Err(GraphReferenceError::Incomplete { .. })
    ));
    assert!(matches!(
        references.exact_export(&invalid_correlation),
        Err(GraphReferenceError::Incomplete { .. })
    ));
}

/// The import half of the pinned-family list: a document naming a family
/// `GraphExport` has no field for is refused, because there is nowhere to
/// put it. Every collaboration and ideation family is tried by name, as are
/// the service families that were deleted from the model.
#[test]
#[verifies("rule_pinned_graph_families", construction)]
fn exact_export_rejects_non_canonical_graph_families() {
    assert!(
        ExactExport::from_json(&serde_json::to_vec(&export_document(&empty_graph())).unwrap())
            .is_ok(),
        "the canonical families alone must be accepted, or the refusals below prove nothing"
    );
    for family in [
        "proposals",
        "proposal_cards",
        "assertions",
        "dispositions",
        "promotion_decisions",
        "contributions",
        "synthesis_packets",
        "threads",
        "messages",
        "services",
        "service_bindings",
    ] {
        let mut graph = empty_graph();
        graph
            .as_object_mut()
            .unwrap()
            .insert(family.into(), json!([]));
        let bytes = serde_json::to_vec(&export_document(&graph)).unwrap();

        let Err(error) = ExactExport::from_json(&bytes) else {
            panic!("family '{family}' travelled in a pinned graph");
        };
        assert!(
            matches!(error, GraphReferenceError::Incomplete { .. }),
            "family '{family}' was not refused as incomplete"
        );
        assert!(
            error.to_string().contains(family),
            "refusal for family '{family}' does not name it: {error}"
        );
    }
}

/// A document carrying `graph`, and the digest that graph hashes to.
///
/// The digest is derived here rather than written down, so no fixture claims a
/// hash its graph does not have and every refusal below is earned by what the
/// test set out to break. A graph the projection cannot hold, because it names
/// a family or a record field that does not exist, has no digest to derive;
/// those documents carry the empty graph's digest and are refused while being
/// read, well before the digest is looked at.
fn export_document(graph: &serde_json::Value) -> serde_json::Value {
    json!({
        "schema_version": 1,
        "operation": "exact-export",
        "reference_id": format!("grf1_{}", "0".repeat(64)),
        "graph_digest": derived_digest(graph),
        "graph": graph
    })
}

fn derived_digest(graph: &serde_json::Value) -> String {
    let graph = serde_json::from_value::<GraphExport>(graph.clone())
        .or_else(|_| serde_json::from_value::<GraphExport>(empty_graph()))
        .expect("the empty graph is a graph");
    graph_digest(&graph).expect("a graph can be canonicalized")
}

fn empty_graph() -> serde_json::Value {
    json!({
        "schema_version": 1,
        "scope": {"id": "default", "path_prefix": "."},
        "sources": [],
        "domains": [],
        "requirements": [],
        "boundaries": [],
        "topics": [],
        "questions": [],
        "resolutions": [],
        "rules": []
    })
}

#[test]
fn exact_export_rejects_unsupported_record_schema_versions() {
    let mut graph = empty_graph();
    graph["sources"] = json!([{
        "schema_version": 2,
        "scope_id": "default",
        "id": "source_policy",
        "name": "Policy",
        "source_type": "policy",
        "url": null
    }]);
    let document = export_document(&graph);

    let error = ExactExport::from_json(&serde_json::to_vec(&document).unwrap()).unwrap_err();
    assert!(matches!(error, GraphReferenceError::Incomplete { .. }));
    assert!(error.to_string().contains("source 'source_policy'"));
    assert!(error.to_string().contains("schema_version 2"));
}

#[test]
fn exact_export_rejection_names_the_unsupported_document_version() {
    let mut document = export_document(&empty_graph());
    document["schema_version"] = json!(2);

    let error = ExactExport::from_json(&serde_json::to_vec(&document).unwrap()).unwrap_err();

    assert!(error
        .to_string()
        .contains("exact export has unsupported schema_version 2; expected 1"));
}

#[test]
fn exact_export_rejection_names_the_unsupported_graph_version() {
    let mut graph = empty_graph();
    graph["schema_version"] = json!(2);
    let document = export_document(&graph);

    let error = ExactExport::from_json(&serde_json::to_vec(&document).unwrap()).unwrap_err();

    assert!(error
        .to_string()
        .contains("graph has unsupported schema_version 2; expected 1"));
}

#[test]
#[verifies("rule_export_strips_collaboration", examples)]
fn exact_export_rejects_collaboration_metadata() {
    let mut graph = empty_graph();
    graph["sources"] = json!([{
        "schema_version": 1,
        "scope_id": "default",
        "id": "source_policy",
        "name": "Policy",
        "source_type": "policy",
        "url": null,
        "origin_thread": "thread_private"
    }]);
    let document = export_document(&graph);

    let error = ExactExport::from_json(&serde_json::to_vec(&document).unwrap()).unwrap_err();
    assert!(matches!(error, GraphReferenceError::Incomplete { .. }));
    assert!(error.to_string().contains("origin_thread"));
}
