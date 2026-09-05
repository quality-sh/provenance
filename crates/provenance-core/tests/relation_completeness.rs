//! The relation vocabulary covers every reference-typed field.
//!
//! `#[derive(Relations)]` refuses a `StableId` field with no attribute,
//! but it reads only the literal spelling: a field typed through a type
//! alias or a wrapper falls through the derive silently, and the serde
//! walk over serialized keys cannot see a second `SourceReference` list
//! on a kind that already declares `cites`, because it serializes to the
//! same `source_id` key. This gate scans the node-kind struct definitions
//! by text instead.
//!
//! What the gate catches: every field whose type spells a reference
//! (`StableId`, `SourceReference`, `ArtifactLink`, or a `type` alias of
//! `StableId` declared in the scanned files) and carries no
//! `#[relation(...)]` attribute, unless it is the record's own `id`.
//!
//! What the gate does not catch, stated plainly: the scan resolves no
//! types, so a newtype wrapper around `StableId` (say
//! `struct RequirementId(StableId)`) or an alias declared outside the
//! scanned files escapes it. A reference hidden behind one would also
//! escape the declaration tables and every check built on them; only a
//! spelling the scan knows can be caught.

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// The files the node-kind structs live in.
const NODE_FILES: &[&str] = &["artifacts.rs", "shaping.rs", "services.rs"];

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

/// Type spellings that mark a field as holding a reference to another
/// record. Aliases of `StableId` declared in the scanned files join this
/// list at run time.
const REFERENCE_SPELLINGS: &[&str] = &["StableId", "SourceReference", "ArtifactLink"];

/// One field the scan saw.
#[derive(Debug, PartialEq, Eq)]
struct ScannedField {
    record: String,
    name: String,
    field_type: String,
    relation_attribute: bool,
}

impl ScannedField {
    fn is_reference_typed(&self, aliases: &[String]) -> bool {
        REFERENCE_SPELLINGS
            .iter()
            .copied()
            .chain(aliases.iter().map(String::as_str))
            .any(|spelling| self.field_type.contains(spelling))
    }

    /// The owner key is the record's own identity, not a reference.
    fn is_owner_key(&self) -> bool {
        self.name == "id"
    }
}

/// The type aliases of `StableId` declared in the scanned sources.
fn stable_id_aliases(sources: &[(String, String)]) -> Vec<String> {
    let mut aliases = Vec::new();
    for (_, source) in sources {
        for line in source.lines() {
            let trimmed = line.trim_start();
            let Some(rest) = trimmed.strip_prefix("pub type ") else {
                continue;
            };
            let Some((name, target)) = rest.split_once('=') else {
                continue;
            };
            if target.trim().starts_with("StableId") {
                aliases.push(name.trim().to_string());
            }
        }
    }
    aliases
}

/// The fields of the named structs, read from the source text.
fn reference_fields(source: &str, structs: &[&str]) -> Vec<ScannedField> {
    let lines: Vec<&str> = source.lines().collect();
    let mut fields = Vec::new();
    for name in structs {
        let declaration = format!("pub struct {name} {{");
        let Some(start) = lines.iter().position(|line| line.contains(&declaration)) else {
            continue;
        };
        let mut body = lines[start + 1..].iter();
        let mut attribute_block = Vec::new();
        for line in body.by_ref() {
            let trimmed = line.trim();
            if trimmed == "}" {
                break;
            }
            let Some(rest) = trimmed.strip_prefix("pub ") else {
                attribute_block.push(trimmed);
                continue;
            };
            let Some((field, field_type)) = rest.split_once(':') else {
                attribute_block.push(trimmed);
                continue;
            };
            fields.push(ScannedField {
                record: (*name).to_string(),
                name: field.trim().to_string(),
                field_type: field_type.trim().trim_end_matches(',').to_string(),
                relation_attribute: attribute_block
                    .iter()
                    .any(|line| line.contains("#[relation")),
            });
            attribute_block.clear();
        }
    }
    fields
}

fn scanned_fields() -> (Vec<ScannedField>, Vec<String>) {
    let root = workspace_root();
    let sources: Vec<(String, String)> = NODE_FILES
        .iter()
        .map(|file| {
            let path = root.join("src/model").join(file);
            (file.to_string(), std::fs::read_to_string(path).unwrap())
        })
        .collect();
    let aliases = stable_id_aliases(&sources);
    let mut fields = Vec::new();
    for (_, source) in &sources {
        fields.extend(reference_fields(source, NODE_STRUCTS));
    }
    (fields, aliases)
}

#[test]
fn the_field_scan_sees_a_hidden_optional_reference() {
    let planted = "pub struct Source {\n    pub id: StableId,\n    pub name: String,\n    #[serde(default)]\n    pub twin_of: Option<StableId>,\n}\n";
    let fields = reference_fields(planted, &["Source"]);
    assert!(
        fields
            .iter()
            .any(|field| field.name == "twin_of" && !field.relation_attribute),
        "an Option<StableId> field with no attribute must be seen: {fields:?}"
    );
    assert!(
        fields.iter().any(|field| field.name == "name"),
        "the scan reports every field; the type decides the reference"
    );
}

#[test]
fn the_field_scan_sees_a_second_via_struct_list_without_an_attribute() {
    let planted = "pub struct Requirement {\n    pub id: StableId,\n    #[relation(target = Source, name = \"cites\", via = source_id)]\n    pub source_refs: Vec<SourceReference>,\n    pub context_refs: Vec<SourceReference>,\n}\n";
    let fields = reference_fields(planted, &["Requirement"]);
    let context = fields
        .iter()
        .find(|field| field.name == "context_refs")
        .unwrap();
    assert!(
        !context.relation_attribute,
        "a second via-struct list with no attribute must be seen: {fields:?}"
    );
}

#[test]
fn the_field_scan_ties_an_attribute_to_its_own_field() {
    let planted = "pub struct Requirement {\n    pub id: StableId,\n    #[relation(target = Source, name = \"cites\", via = source_id)]\n    pub source_refs: Vec<SourceReference>,\n}\n";
    let fields = reference_fields(planted, &["Requirement"]);
    let cited = fields
        .iter()
        .find(|field| field.name == "source_refs")
        .unwrap();
    assert!(
        cited.relation_attribute,
        "the attribute above the field answers for it: {fields:?}"
    );
}

#[test]
fn an_alias_of_stable_id_is_a_reference_spelling() {
    let planted = "pub type ClaimId = StableId;\n\npub struct Requirement {\n    pub id: StableId,\n    pub claim: ClaimId,\n}\n";
    let aliases = stable_id_aliases(&[("planted".into(), planted.to_string())]);
    assert_eq!(aliases, ["ClaimId"]);
    let fields = reference_fields(planted, &["Requirement"]);
    let claim = fields.iter().find(|field| field.name == "claim").unwrap();
    assert!(
        claim.is_reference_typed(&aliases) && !claim.relation_attribute,
        "a field typed through an alias with no attribute must be caught: {fields:?}"
    );
}

#[test]
fn a_newtype_wrapper_escapes_the_scan_and_the_comment_says_so() {
    let module =
        std::fs::read_to_string(workspace_root().join("tests/relation_completeness.rs")).unwrap();
    assert!(
        module.contains("newtype wrapper around `StableId`"),
        "the gate must state its own blind spot"
    );
}

/// The one exemption by name: `links` is its own entry in the relation
/// vocabulary (LINKS), read from the record by hand rather than declared
/// with a `#[relation]` attribute. Traversal, the gap report, and the
/// derived relation table all walk it explicitly.
fn is_hand_walked_links(field: &ScannedField) -> bool {
    field.name == "links" && field.field_type.contains("ArtifactLink")
}

#[test]
fn every_reference_field_carries_a_relation_attribute() {
    let (fields, aliases) = scanned_fields();
    let reference_fields: Vec<&ScannedField> = fields
        .iter()
        .filter(|field| field.is_reference_typed(&aliases) && !field.is_owner_key())
        .collect();
    assert!(
        reference_fields.len() >= 20,
        "the scan must see the node structs; saw {reference_fields:?}"
    );
    let unanswered: Vec<String> = reference_fields
        .iter()
        .filter(|field| !field.relation_attribute && !is_hand_walked_links(field))
        .map(|field| format!("{}.{}", field.record, field.name))
        .collect();
    assert!(
        unanswered.is_empty(),
        "reference fields without a relation attribute: {unanswered:?}"
    );
    let owner_keys = fields.iter().filter(|field| field.is_owner_key()).count();
    assert_eq!(
        owner_keys,
        NODE_STRUCTS.len(),
        "every node struct must be scanned for its owner key; scanned {fields:?}"
    );
}
