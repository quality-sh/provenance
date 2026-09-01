//! The relation vocabulary covers every reference-typed field.
//!
//! A record-to-record reference hides easily inside an `Option<StableId>`
//! field. This gate scans the node-kind struct definitions for fields whose
//! type carries a reference (`StableId`, `SourceReference`, `ArtifactLink`)
//! and demands that each one is either mapped to a declared `RelationKind`
//! or exempted here with a written reason. A new reference field fails this
//! test until the vocabulary answers for it.

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

const NODE_STRUCTS: &[&str] = &[
    "Source",
    "Requirement",
    "Domain",
    "Boundary",
    "Topic",
    "Question",
    "Resolution",
    "Rule",
];

/// (struct, field) -> the relation that answers for it.
const DECLARED: &[(&str, &str, &str)] = &[
    ("Source", "superseded_by", "source_superseded_by"),
    ("Requirement", "domain_id", "requirement_in_domain"),
    ("Requirement", "source_refs", "requirement_cites_source"),
    ("Boundary", "requirement_id", "boundary_constrains"),
    ("Boundary", "source_ref", "boundary_cites_source"),
    ("Topic", "requirement_id", "topic_shapes"),
    ("Topic", "links", "topic_links"),
    ("Question", "topic_id", "question_belongs_to_topic"),
    ("Question", "requirement_id", "question_refines"),
    ("Question", "resolution_id", "question_settled_by"),
    ("Question", "links", "question_links"),
    ("Resolution", "superseded_by", "resolution_superseded_by"),
];

/// (struct, field) -> why no relation answers for it.
const EXEMPT: &[(&str, &str, &str)] = &[
    ("Source", "id", "a record's own identity is not a reference"),
    (
        "Requirement",
        "id",
        "a record's own identity is not a reference",
    ),
    ("Domain", "id", "a record's own identity is not a reference"),
    (
        "Boundary",
        "id",
        "a record's own identity is not a reference",
    ),
    ("Topic", "id", "a record's own identity is not a reference"),
    (
        "Question",
        "id",
        "a record's own identity is not a reference",
    ),
    (
        "Resolution",
        "id",
        "a record's own identity is not a reference",
    ),
    ("Rule", "id", "a record's own identity is not a reference"),
    (
        "Source",
        "origin_thread",
        "threads are collaboration records, not graph nodes",
    ),
    (
        "Source",
        "origin_message",
        "messages are collaboration records, not graph nodes",
    ),
    (
        "Requirement",
        "origin_thread",
        "threads are collaboration records, not graph nodes",
    ),
    (
        "Requirement",
        "origin_message",
        "messages are collaboration records, not graph nodes",
    ),
    (
        "Resolution",
        "origin_thread",
        "threads are collaboration records, not graph nodes",
    ),
    (
        "Resolution",
        "origin_message",
        "messages are collaboration records, not graph nodes",
    ),
    (
        "Rule",
        "origin_thread",
        "threads are collaboration records, not graph nodes",
    ),
    (
        "Rule",
        "origin_message",
        "messages are collaboration records, not graph nodes",
    ),
];

/// Reference-typed fields of the named structs, read from source text.
fn reference_fields(source: &str, structs: &[&str]) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    for name in structs {
        let declaration = format!("pub struct {name} {{");
        let Some(start) = source.find(&declaration) else {
            continue;
        };
        let body_start = start + declaration.len();
        let Some(length) = source[body_start..].find("\n}") else {
            continue;
        };
        for line in source[body_start..body_start + length].lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("pub ") else {
                continue;
            };
            let Some((field, field_type)) = rest.split_once(':') else {
                continue;
            };
            if ["StableId", "SourceReference", "ArtifactLink"]
                .iter()
                .any(|reference| field_type.contains(reference))
            {
                fields.push(((*name).to_string(), field.trim().to_string()));
            }
        }
    }
    fields
}

#[test]
fn the_field_scan_sees_a_hidden_optional_reference() {
    let planted = "pub struct Source {\n    pub id: StableId,\n    pub name: String,\n    pub twin_of: Option<StableId>,\n}\n";
    let fields = reference_fields(planted, &["Source"]);
    assert!(
        fields.contains(&("Source".into(), "twin_of".into())),
        "an Option<StableId> field must be seen: {fields:?}"
    );
    assert!(!fields.contains(&("Source".into(), "name".into())));
}

#[test]
fn every_reference_field_is_declared_or_exempted_with_a_reason() {
    let root = workspace_root();
    let mut fields = Vec::new();
    for file in ["artifacts.rs", "services.rs", "shaping.rs"] {
        let source = std::fs::read_to_string(root.join("src/model").join(file)).unwrap();
        fields.extend(reference_fields(&source, NODE_STRUCTS));
    }
    assert!(
        fields.len() >= 20,
        "the scan must see the node structs; saw {fields:?}"
    );
    let mut unanswered = Vec::new();
    for (record, field) in &fields {
        let declared = DECLARED
            .iter()
            .any(|(kind, name, _)| kind == record && name == field);
        let exempted = EXEMPT
            .iter()
            .any(|(kind, name, _)| kind == record && name == field);
        if !declared && !exempted {
            unanswered.push(format!("{record}.{field}"));
        }
    }
    assert!(
        unanswered.is_empty(),
        "reference fields without a declared relation or a written exemption: {unanswered:?}"
    );
    for (record, field, relation) in DECLARED {
        assert!(
            fields
                .iter()
                .any(|(kind, name)| kind == record && name == field),
            "declared mapping {record}.{field} -> {relation} names a field the scan cannot see"
        );
    }
}
