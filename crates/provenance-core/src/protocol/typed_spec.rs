//! The language-neutral typed-spec input family.
//!
//! These are wire shapes: every SDK frontend sends them and the engine
//! decodes them. They live in core beside the other protocol types.
//! `Serialize` is for fixture emission and kernel materialization; the
//! wire decoder keeps `deny_unknown_fields`.

use provenance_macros::rule;
use serde::{Deserialize, Serialize};

use crate::model::DeclarationAddress;

/// One language-authored desired-state document.
///
/// Serialization skips absent optional fields, so a decode and encode
/// round trip preserves every present field and every omission.
#[rule("rule_rust_typed_input_round_trip")]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSpecInput {
    pub schema_version: u32,
    pub spec: String,
    pub declared_by: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adopt_unowned: Vec<TypedAdoptionTarget>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<TypedSourceInput>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirements: Vec<TypedRequirementInput>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<TypedRuleInput>,
}

/// One exact declaration identity that may transition from unowned to owned.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypedAdoptionTarget {
    pub kind: TypedDeclarationKind,
    pub id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TypedDeclarationKind {
    Source,
    Requirement,
    Rule,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSourceInput {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypedRequirementInput {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub statement: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypedRuleInput {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<DeclarationAddress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirements: Vec<String>,
    pub statement: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation: Option<TypedImplementationInput>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypedImplementationInput {
    pub file: camino::Utf8PathBuf,
    pub symbol: String,
}

/// The fixed request shape for one statement preflight.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckStatementRequest {
    pub statement: String,
}
