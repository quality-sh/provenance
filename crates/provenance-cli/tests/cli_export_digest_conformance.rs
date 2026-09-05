//! Two readings of an exact-export document's `graph_digest`, held together.
//!
//! The document travels alone, so two things read its digest and they can
//! drift apart. `provenance schema show graph-reference-export` publishes a
//! JSON Schema that a holder outside this codebase checks the document
//! against; `ExactExport::from_json` is what this codebase checks it with.
//! Every boundary value of the digest field is pushed through both, and the
//! test says which side accepted and which refused.
//!
//! The two are not the same check, and this test does not pretend they are.
//! The schema sees shape: present, a string, `sha256:` and sixty-four
//! lowercase hex digits. It cannot see whether the digest is the hash of the
//! graph beside it, because a JSON Schema cannot hash anything. So the
//! agreement asserted below is agreement on shape, and the one case the
//! schema cannot see - a well-formed digest belonging to some other graph -
//! is asserted as a split: the schema accepts it, and the runtime alone
//! refuses it, naming the field.
//!
//! Deliberately unmarked by `#[verifies]`. `rule_reference_verified` is
//! about a reference checked against the repository it names (its commit
//! exists, the repository identity matches, the id is the hash of those
//! parts), and nothing here touches a repository. Nor is this a conformance
//! check in the sense the marker carries elsewhere, where a published schema
//! is a replica of the rule: the schema is a replica of the digest's shape
//! only, and the split case below is the proof it is not more than that.
//! Marking it would claim verification this test does not do.

use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_store::graph_reference::{
    graph_digest, ExactExport, GraphExport, GraphReferenceError,
};
use serde_json::{json, Value};

/// A pinned graph carrying one named source and nothing else. The name is
/// what makes two graphs hash differently.
fn graph_with_source_named(name: &str) -> Value {
    json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope": {"id": "default", "path_prefix": "."},
        "sources": [{
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": "default",
            "id": "source_policy",
            "name": name,
            "source_type": "policy",
            "url": null
        }],
        "domains": [],
        "requirements": [],
        "boundaries": [],
        "topics": [],
        "questions": [],
        "resolutions": [],
        "rules": []
    })
}

fn digest_of(graph: &Value) -> String {
    let parsed: GraphExport =
        serde_json::from_value(graph.clone()).expect("the fixture graph is a pinned graph");
    graph_digest(&parsed).expect("a graph can be canonicalized")
}

/// An exact-export document carrying `graph` and the digest that graph
/// hashes to, so the only claim any case below breaks is the one it names.
fn export_document(graph: &Value) -> Value {
    json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "operation": "exact-export",
        "reference_id": format!("grf1_{}", "0".repeat(64)),
        "graph_digest": digest_of(graph),
        "graph": graph
    })
}

fn export_validator() -> jsonschema::JSONSchema {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "schema",
            "show",
            "graph-reference-export",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "schema show failed");
    let shown: Value = serde_json::from_slice(&output.stdout).unwrap();
    jsonschema::JSONSchema::compile(shown.get("schema").unwrap()).unwrap()
}

struct Case {
    name: &'static str,
    /// What to write at `graph_digest`, or `None` to leave the field out.
    digest: Option<Value>,
    /// Whether the digest is a well-formed `sha256:` digest, which is all
    /// the schema can decide.
    shape_legal: bool,
    /// Whether it is also the digest this document's graph hashes to, which
    /// only the runtime can decide.
    digest_correct: bool,
}

const fn shape_refused(name: &'static str, digest: Option<Value>) -> Case {
    Case {
        name,
        digest,
        shape_legal: false,
        digest_correct: false,
    }
}

fn cases(correct: &str, other_graph_digest: &str) -> Vec<Case> {
    let hex = "0".repeat(64);
    vec![
        Case {
            name: "correct-digest",
            digest: Some(json!(correct)),
            shape_legal: true,
            digest_correct: true,
        },
        shape_refused("missing-digest", None),
        shape_refused("null-digest", Some(Value::Null)),
        shape_refused("numeric-digest", Some(json!(0))),
        shape_refused("empty-digest", Some(json!(""))),
        shape_refused("prefix-only", Some(json!("sha256:"))),
        shape_refused("no-prefix", Some(json!(hex.clone()))),
        shape_refused("wrong-prefix", Some(json!(format!("sha1:{hex}")))),
        shape_refused("uppercase-prefix", Some(json!(format!("SHA256:{hex}")))),
        shape_refused(
            "63-digits",
            Some(json!(format!("sha256:{}", "0".repeat(63)))),
        ),
        shape_refused(
            "65-digits",
            Some(json!(format!("sha256:{}", "0".repeat(65)))),
        ),
        shape_refused(
            "uppercase-digits",
            Some(json!(format!("sha256:{}", "A".repeat(64)))),
        ),
        shape_refused(
            "non-hex-digits",
            Some(json!(format!("sha256:{}", "g".repeat(64)))),
        ),
        shape_refused("leading-space", Some(json!(format!(" sha256:{hex}")))),
        shape_refused("trailing-newline", Some(json!(format!("sha256:{hex}\n")))),
        // The two the schema cannot decide: well-formed digests that are not
        // this graph's. One is a hash no graph has, the other is the hash
        // another graph really has.
        Case {
            name: "well-formed-digest-of-nothing",
            digest: Some(json!(format!("sha256:{hex}"))),
            shape_legal: true,
            digest_correct: false,
        },
        Case {
            name: "well-formed-digest-of-another-graph",
            digest: Some(json!(other_graph_digest)),
            shape_legal: true,
            digest_correct: false,
        },
    ]
}

fn candidate(document: &Value, case: &Case) -> Value {
    let mut candidate = document.clone();
    let object = candidate.as_object_mut().unwrap();
    match case.digest.clone() {
        Some(value) => {
            object.insert("graph_digest".to_string(), value);
        }
        None => {
            object.remove("graph_digest");
        }
    }
    candidate
}

const fn verdict(accepted: bool) -> &'static str {
    if accepted {
        "accepted"
    } else {
        "refused"
    }
}

#[test]
fn export_schema_and_runtime_agree_on_digest_shape_and_only_the_runtime_catches_a_wrong_digest() {
    let graph = graph_with_source_named("Policy");
    let document = export_document(&graph);
    let correct = digest_of(&graph);
    let other = digest_of(&graph_with_source_named("Another policy"));
    assert_ne!(correct, other, "the two fixture graphs must hash apart");
    let validator = export_validator();

    let mut splits = 0;
    for case in cases(&correct, &other) {
        let candidate = candidate(&document, &case);
        let schema_accepts = validator.is_valid(&candidate);
        let runtime = ExactExport::from_json(&serde_json::to_vec(&candidate).unwrap());

        assert_eq!(
            schema_accepts,
            case.shape_legal,
            "the schema {} {}, which is {}",
            verdict(schema_accepts),
            case.name,
            verdict(case.shape_legal)
        );
        assert_eq!(
            runtime.is_ok(),
            case.shape_legal && case.digest_correct,
            "the runtime {} {}: {:?}",
            verdict(runtime.is_ok()),
            case.name,
            runtime.as_ref().err()
        );

        if case.shape_legal && !case.digest_correct {
            splits += 1;
            assert!(
                matches!(
                    runtime,
                    Err(GraphReferenceError::Mismatched {
                        field: "graph_digest",
                        ..
                    })
                ),
                "the runtime should refuse {} as a mismatched graph_digest, got {runtime:?}",
                case.name
            );
        } else {
            assert_eq!(
                schema_accepts,
                runtime.is_ok(),
                "schema and runtime disagree on {}: schema {}, runtime {}",
                case.name,
                verdict(schema_accepts),
                verdict(runtime.is_ok())
            );
        }
    }
    assert_eq!(
        splits, 2,
        "both well-formed-but-wrong digests must reach the runtime alone"
    );
}
