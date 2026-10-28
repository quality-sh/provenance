//! The id character set has one home in the runtime — `is_well_formed_id` in
//! `crates/provenance-core/src/model/ids.rs` — and four replicas in the JSON
//! Schemas this CLI publishes. A holder outside this codebase never runs the
//! runtime; it runs the replica. If the two disagree, a document the schema
//! calls valid is refused on import, or worse, the other way round.
//!
//! So this test does not read the replicas out of the source files. It asks
//! the shipped binary for the schemas it actually emits, pulls the four
//! `pattern` strings out of them, and pushes a boundary corpus through both
//! every replica and both runtime constructors. A disagreement names the
//! value, the replica, and which side accepted it.

use assert_cmd::Command;
use provenance_core::{ScopeId, StableId};
use provenance_macros::verifies;
use serde_json::{json, Value};

/// One published copy of the character set: where it lives in the source, and
/// the pattern string the binary emitted for it.
struct Replica {
    origin: &'static str,
    pattern: String,
    validator: jsonschema::Validator,
}

impl Replica {
    fn accepts(&self, candidate: &str) -> bool {
        self.validator.is_valid(&json!(candidate))
    }
}

fn shown_schema(artifact: &str) -> Value {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args(["schema", "show", artifact, "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success(), "schema show {artifact} failed");
    serde_json::from_slice(&output.stdout).unwrap()
}

fn pattern_at(schema: &Value, pointer: &str) -> String {
    schema
        .pointer(pointer)
        .unwrap_or_else(|| panic!("no pattern at {pointer}; the schema shape moved"))
        .as_str()
        .unwrap()
        .to_string()
}

/// The four replicas, read out of the emitted schemas rather than the source.
fn replicas() -> Vec<Replica> {
    let reference = shown_schema("graph-reference");
    let export = shown_schema("graph-reference-export");
    [
        (
            "schema/common/mod.rs $defs.stableId",
            pattern_at(&reference, "/$defs/stableId/pattern"),
        ),
        (
            "schema/common/mod.rs $defs.scopeId",
            pattern_at(&reference, "/$defs/scopeId/pattern"),
        ),
        (
            "schema/artifacts/graph_reference.rs reference_schema scope_id",
            pattern_at(&reference, "/schema/properties/scope_id/pattern"),
        ),
        (
            "schema/artifacts/graph_reference.rs export_definitions id",
            pattern_at(&export, "/schema/$defs/scope/properties/id/pattern"),
        ),
    ]
    .into_iter()
    .map(|(origin, pattern)| {
        let schema = json!({"type": "string", "pattern": pattern});
        let validator = jsonschema::draft7::new(&schema)
            .unwrap_or_else(|error| panic!("{origin} publishes an uncompilable pattern: {error}"));
        Replica {
            origin,
            pattern,
            validator,
        }
    })
    .collect()
}

/// Boundary values, named so a failure reads as prose. Every id-shaped edge
/// the rule has an opinion about: the empty string, each allowed character
/// class alone, hyphen and underscore at both ends, case, whitespace,
/// punctuation, control characters, non-ASCII that looks ASCII, and length.
fn corpus() -> Vec<(String, String)> {
    let mut cases: Vec<(String, String)> = [
        ("single letter", "a"),
        ("last letter", "z"),
        ("single digit", "0"),
        ("last digit", "9"),
        ("underscore alone", "_"),
        ("hyphen alone", "-"),
        ("ordinary scope id", "default"),
        ("ordinary stable id", "source_codebase"),
        ("internal hyphen", "a-b"),
        ("internal underscore", "a_b"),
        ("leading hyphen", "-a"),
        ("trailing hyphen", "a-"),
        ("leading underscore", "_a"),
        ("trailing underscore", "a_"),
        ("doubled hyphen", "a--b"),
        ("all classes mixed", "a1-b_2"),
        ("empty string", ""),
        ("single uppercase", "A"),
        ("leading uppercase", "Default"),
        ("camel case", "sourceCodebase"),
        ("trailing uppercase", "defaulT"),
        ("space alone", " "),
        ("internal space", "a b"),
        ("trailing space", "a "),
        ("leading space", " a"),
        ("tab", "a\tb"),
        ("trailing newline", "a\n"),
        ("leading newline", "\na"),
        ("internal newline", "a\nb"),
        ("newline after a bad line", "A\na"),
        ("carriage return", "a\r"),
        ("slash", "source/codebase"),
        ("dot", "a.b"),
        ("colon", "a:b"),
        ("plus", "a+b"),
        ("at sign", "a@b"),
        ("asterisk", "a*"),
        ("apostrophe", "a'b"),
        ("backslash", "a\\b"),
        ("percent escape", "a%2fb"),
        ("null character", "a\u{0}b"),
        ("accented latin", "café"),
        ("cjk", "日本"),
        ("fullwidth latin a", "ａ"),
        ("cyrillic a", "а"),
        ("dotted capital i", "İ"),
        ("zero width space", "a\u{200b}b"),
        ("combining accent", "a\u{301}"),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_string(), value.to_string()))
    .collect();
    cases.push(("very long id".to_string(), "a".repeat(10_000)));
    cases.push((
        "very long id with one uppercase".to_string(),
        format!("{}A{}", "a".repeat(5_000), "a".repeat(5_000)),
    ));
    cases
}

const fn verdict(accepted: bool) -> &'static str {
    if accepted {
        "accepted"
    } else {
        "refused"
    }
}

#[test]
#[verifies("rule_id_charset", conformance)]
fn every_published_schema_replica_agrees_with_the_runtime_on_every_boundary_value() {
    let replicas = replicas();
    assert_eq!(replicas.len(), 4, "a replica went missing from the schemas");

    for (name, candidate) in corpus() {
        let stable = StableId::new(candidate.clone()).is_ok();
        let scope = ScopeId::new(candidate.clone()).is_ok();
        assert_eq!(
            stable,
            scope,
            "the two runtime constructors disagree on {name} ({candidate:?}): \
             StableId {}, ScopeId {}",
            verdict(stable),
            verdict(scope)
        );

        for replica in &replicas {
            assert_eq!(
                replica.accepts(&candidate),
                stable,
                "{} publishes {} which {} {name} ({candidate:?}), \
                 but the runtime {} it",
                replica.origin,
                replica.pattern,
                verdict(replica.accepts(&candidate)),
                verdict(stable)
            );
        }
    }
}

#[test]
#[verifies("rule_id_charset", conformance)]
fn all_four_replicas_publish_the_same_pattern() {
    let replicas = replicas();
    let first = &replicas[0];
    for replica in &replicas[1..] {
        assert_eq!(
            replica.pattern, first.pattern,
            "{} publishes {} but {} publishes {}",
            replica.origin, replica.pattern, first.origin, first.pattern
        );
    }
}
