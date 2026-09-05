use super::*;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_macros::verifies;
use serde_json::json;

#[test]
fn graph_reference_schemas_are_exposed() {
    let temp = committed_store();
    for artifact in ["graph-reference", "graph-reference-export"] {
        provenance(temp.path())
            .args(["schema", "show", artifact, "--format", "json"])
            .assert()
            .success()
            .stdout(predicate::str::contains(format!(
                "\"const\": {}",
                SUPPORTED_SCHEMA_VERSION.0
            )))
            .stdout(predicate::str::contains("additionalProperties"));
    }
    provenance(temp.path())
        .args([
            "schema",
            "show",
            "graph-reference-export",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"retired\""));
}

#[test]
fn emitted_graph_reference_schema_validates_an_issued_reference() {
    let temp = committed_store();
    let reference = issue(temp.path(), &[]);
    let validator = reference_validator(temp.path());

    assert!(validator.is_valid(&reference));
}

/// The JSON Schema published by `schema show graph-reference` is a replica of
/// `GraphReference::validate`: it is what a holder outside this codebase checks
/// a reference against. Every boundary value of the rule is pushed through both
/// the replica and the runtime validator, and the two must reach the same
/// verdict on each. A disagreement is a drift between the rule and its replica,
/// so the test names which side accepted and which refused.
#[test]
#[verifies("rule_reference_wellformed", conformance)]
fn graph_reference_schema_and_runtime_reject_the_same_boundary_values() {
    let temp = committed_store();
    let validator = reference_validator(temp.path());
    let reference = issue(temp.path(), &[]);

    for (name, well_formed, candidate) in boundary_values(&reference) {
        let schema_accepts = validator.is_valid(&candidate);
        let runtime_accepts = runtime_accepts(temp.path(), &name, &candidate);
        assert_eq!(
            schema_accepts,
            runtime_accepts,
            "schema and runtime disagree on {name}: \
             schema {}, runtime {}",
            verdict(schema_accepts),
            verdict(runtime_accepts)
        );
        assert_eq!(
            schema_accepts,
            well_formed,
            "{name} should be {}, both sides said {}",
            verdict(well_formed),
            verdict(schema_accepts)
        );
    }
}

const fn verdict(accepted: bool) -> &'static str {
    if accepted {
        "accepted"
    } else {
        "refused"
    }
}

fn reference_validator(repo: &Path) -> jsonschema::JSONSchema {
    let output = provenance(repo)
        .args(["schema", "show", "graph-reference", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let shown: Value = serde_json::from_slice(&output).unwrap();
    jsonschema::JSONSchema::compile(shown.get("schema").unwrap()).unwrap()
}

fn runtime_accepts(repo: &Path, name: &str, candidate: &Value) -> bool {
    let path = repo.join(format!("{name}.json"));
    std::fs::write(&path, serde_json::to_vec(candidate).unwrap()).unwrap();
    provenance(repo)
        .args([
            "validate",
            "graph-reference",
            "--input",
            path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap()
        .status
        .success()
}

fn digits(count: usize) -> String {
    "0".repeat(count)
}

/// Every boundary value of the well-formedness rule, as
/// (name, well-formed, document).
fn boundary_values(reference: &Value) -> Vec<(String, bool, Value)> {
    let mut cases = vec![
        ("issued-reference".to_string(), true, reference.clone()),
        ("missing-commit".to_string(), false, {
            let mut candidate = reference.clone();
            candidate.as_object_mut().unwrap().remove("commit");
            candidate
        }),
        ("unknown-field".to_string(), false, {
            let mut candidate = reference.clone();
            candidate["issued_by"] = json!("someone");
            candidate
        }),
    ];
    for (name, well_formed, field, value) in field_boundaries() {
        let mut candidate = reference.clone();
        candidate[field] = value;
        cases.push((name, well_formed, candidate));
    }
    cases
}

/// The one boundary value where the replica cannot follow the runtime.
///
/// `schema_version: 1.0` is refused by the runtime, which reads the field into
/// a `u32`. The published schema says `{"type": "integer", "const": 1}` and
/// still accepts it: JSON Schema from draft 6 on has one number type, compares
/// `const` by value, and counts any number with no fractional part as an
/// integer, so `1.0` and `1` are the same instance to a conforming validator
/// and no keyword can separate them. The divergence is therefore a property of
/// the language, not a drift in the replica, and it is pinned here rather than
/// left in the shared table, which requires the two sides to agree.
///
/// A holder that writes `1.0` passes the schema and is refused by this
/// codebase. The runtime is the tighter side and it is the one that decides.
#[test]
#[verifies("rule_reference_wellformed", examples)]
fn schema_version_float_is_where_the_replica_is_looser_than_the_runtime() {
    let temp = committed_store();
    let validator = reference_validator(temp.path());
    let mut candidate = issue(temp.path(), &[]);
    candidate["schema_version"] =
        serde_json::from_str(&format!("{}.0", SUPPORTED_SCHEMA_VERSION.0)).unwrap();

    assert!(
        validator.is_valid(&candidate),
        "JSON Schema cannot refuse 1.0 while accepting 1; if it now does, \
         put the case back in the shared boundary table"
    );
    assert!(!runtime_accepts(
        temp.path(),
        "schema-version-float",
        &candidate
    ));
}

#[allow(clippy::too_many_lines)]
fn field_boundaries() -> Vec<(String, bool, &'static str, Value)> {
    let single_fields: Vec<(&str, bool, &'static str, Value)> = vec![
        (
            "schema-version-below",
            false,
            "schema_version",
            json!(SUPPORTED_SCHEMA_VERSION.0 - 1),
        ),
        (
            "schema-version-above",
            false,
            "schema_version",
            json!(SUPPORTED_SCHEMA_VERSION.0 + 1),
        ),
        (
            "schema-version-string",
            false,
            "schema_version",
            json!(SUPPORTED_SCHEMA_VERSION.0.to_string()),
        ),
        (
            "store-path-trailing-slash",
            false,
            "store_path",
            json!(".provenance/state/"),
        ),
        (
            "store-path-unrooted",
            false,
            "store_path",
            json!("provenance/state"),
        ),
        (
            "store-path-uppercase",
            false,
            "store_path",
            json!(".provenance/State"),
        ),
        ("store-path-empty", false, "store_path", json!("")),
        ("scope-punctuated", true, "scope_id", json!("scope-1_a")),
        ("scope-empty", false, "scope_id", json!("")),
        ("scope-uppercase", false, "scope_id", json!("Default")),
        ("scope-spaced", false, "scope_id", json!("de fault")),
        ("scope-pathlike", false, "scope_id", json!("default/child")),
        ("scope-non-ascii", false, "scope_id", json!("défaut")),
        ("commit-39-digits", false, "commit", json!(digits(39))),
        ("commit-40-digits", true, "commit", json!(digits(40))),
        ("commit-41-digits", false, "commit", json!(digits(41))),
        ("commit-63-digits", false, "commit", json!(digits(63))),
        ("commit-64-digits", true, "commit", json!(digits(64))),
        ("commit-65-digits", false, "commit", json!(digits(65))),
        ("commit-empty", false, "commit", json!("")),
        (
            "commit-uppercase-40",
            false,
            "commit",
            json!("A".repeat(40)),
        ),
        (
            "commit-uppercase-64",
            false,
            "commit",
            json!("A".repeat(64)),
        ),
        ("commit-non-hex", false, "commit", json!("g".repeat(40))),
        (
            "commit-trailing-newline",
            false,
            "commit",
            json!(digits(40) + "\n"),
        ),
        ("correlation-null", false, "correlation", Value::Null),
        ("correlation-empty", false, "correlation", json!({})),
        (
            "correlation-filled",
            true,
            "correlation",
            json!({"system": "github", "key": "issue-35"}),
        ),
        (
            "correlation-padded",
            true,
            "correlation",
            json!({"system": "  github  ", "key": "issue-35"}),
        ),
        (
            "correlation-without-key",
            false,
            "correlation",
            json!({"system": "github"}),
        ),
        (
            "correlation-without-system",
            false,
            "correlation",
            json!({"key": "issue-35"}),
        ),
        (
            "correlation-empty-system",
            false,
            "correlation",
            json!({"system": "", "key": "issue-35"}),
        ),
        (
            "correlation-blank-system",
            false,
            "correlation",
            json!({"system": "   ", "key": "issue-35"}),
        ),
        (
            "correlation-blank-key",
            false,
            "correlation",
            json!({"system": "github", "key": "  "}),
        ),
        (
            "correlation-non-breaking-space-system",
            false,
            "correlation",
            json!({"system": "\u{a0}", "key": "issue-35"}),
        ),
        (
            "correlation-extra-field",
            false,
            "correlation",
            json!({"system": "github", "key": "issue-35", "note": "x"}),
        ),
    ];
    let mut boundaries: Vec<(String, bool, &'static str, Value)> = single_fields
        .into_iter()
        .map(|(name, well_formed, field, value)| (name.to_string(), well_formed, field, value))
        .collect();
    boundaries.extend(prefixed_hash_boundaries(
        "reference-id",
        "reference_id",
        "grf1_",
        "git1_",
    ));
    boundaries.extend(prefixed_hash_boundaries(
        "repository-id",
        "repository_id",
        "git1_",
        "grf1_",
    ));
    boundaries.extend(prefixed_hash_boundaries(
        "graph-digest",
        "graph_digest",
        "sha256:",
        "sha1:",
    ));
    boundaries
}

/// The three prefixed 64-hex identities differ only in their prefix, so their
/// boundaries are the same seven values with the prefix swapped in.
fn prefixed_hash_boundaries(
    name: &str,
    field: &'static str,
    prefix: &str,
    other_prefix: &str,
) -> Vec<(String, bool, &'static str, Value)> {
    let case = |suffix: &str, well_formed: bool, value: String| {
        (format!("{name}-{suffix}"), well_formed, field, json!(value))
    };
    vec![
        case("64-digits", true, prefix.to_string() + &digits(64)),
        case("63-digits", false, prefix.to_string() + &digits(63)),
        case("65-digits", false, prefix.to_string() + &digits(65)),
        case("uppercase", false, prefix.to_string() + &"A".repeat(64)),
        case(
            "wrong-prefix",
            false,
            other_prefix.to_string() + &digits(64),
        ),
        case("no-prefix", false, digits(64)),
        case("prefix-only", false, prefix.to_string()),
    ]
}
