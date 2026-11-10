//! The write gate for merged shards.
//!
//! `merge_records` decides which record survives while treating every record as
//! opaque JSON, so a merge writes records that were never checked here: they
//! come from another branch, an older writer, or a hand edit, and the merge is
//! the moment they enter this repository's shard. A per-record check like the
//! required relation list can only fail on a record some side already held,
//! but holding it is exactly what a merge would launder into canonical state.
//! A check spanning the whole file can fail on the merge's own doing; the one
//! such check that exists today, the duplicate id, is refused by `index_by_id`
//! inside the merge itself.
//!
//! So before a merged shard is written, its records go back through the check a
//! direct write would face, and a record that fails it stops the merge instead
//! of landing in the state directory.

use anyhow::Context;
use camino::Utf8Path;
use provenance_core::model::relations::{missing_required, required_refusal, RelationOwner};
use provenance_core::{Question, Requirement, RequirementReview, Resolution, Rule, Source, Topic};
use serde_json::Value;

use super::CanonicalRecord;
use crate::statement_analysis::{analyze_changed_statements, StatementDiagnostic};

/// The record type a shard holds, read off the repository path of the file
/// being merged.
///
/// The path is the only thing the merge driver knows about the file: git hands
/// the driver three temporary files, so only the merged-result path (`%P`)
/// names the shard. Every state shard sits in a directory named after its
/// family, which is what the recognizers below match on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShardFamily {
    /// `.provenance/state/scopes/<scope>/ideation/landings.jsonl`
    IdeationLandings,
    /// `.provenance/state/scopes/<scope>/threads/discussions.jsonl`
    Discussions,
    /// `.provenance/state/scopes/<scope>/sources/*.jsonl`
    Sources,
    /// `.provenance/state/scopes/<scope>/requirements/review.jsonl`
    RequirementReviews,
    /// `.provenance/state/scopes/<scope>/requirements/*.jsonl`, except the
    /// review file
    Requirements,
    /// `.provenance/state/scopes/<scope>/resolutions/*.jsonl`
    Resolutions,
    /// `.provenance/state/scopes/<scope>/rules/*.jsonl`
    Rules,
    /// `.provenance/state/scopes/<scope>/topics/*.jsonl`
    Topics,
    /// `.provenance/state/scopes/<scope>/questions/*.jsonl`
    Questions,
    /// Any other path, including per-scope families without declared
    /// relations and files outside the state directory. Merged records pass
    /// unchecked.
    Unrecognized,
}

const SCOPED_FAMILIES: [(&str, ShardFamily); 6] = [
    ("sources", ShardFamily::Sources),
    ("requirements", ShardFamily::Requirements),
    ("resolutions", ShardFamily::Resolutions),
    ("rules", ShardFamily::Rules),
    ("topics", ShardFamily::Topics),
    ("questions", ShardFamily::Questions),
];

impl ShardFamily {
    /// Discussion records use `discussion_id`; all other records use `id`.
    #[provenance_macros::rule("rule_discussion_merge_keys_discussion_id")]
    #[must_use]
    pub const fn record_key(self) -> &'static str {
        match self {
            Self::Discussions => "discussion_id",
            _ => "id",
        }
    }

    /// Recognizes the family from the path the merged result will be stored at.
    ///
    /// The recognizers inspect the trailing canonical state layout, including
    /// the `state/scopes/<scope>` ancestors for scoped families. An absolute
    /// path, a repository-relative path, and a path inside a test fixture all
    /// resolve the same way.
    #[must_use]
    pub fn for_shard_path(path: &Utf8Path) -> Self {
        let Some(directory) = path.parent() else {
            return Self::Unrecognized;
        };
        // The review rows are in the requirements directory, but they are not
        // Requirement records.
        if path.file_name() == Some("review.jsonl") && is_scoped_family(path, "requirements") {
            return Self::RequirementReviews;
        }
        if let Some((_, family)) = SCOPED_FAMILIES
            .iter()
            .find(|(name, _)| is_scoped_family(path, name))
        {
            return *family;
        }
        if path.file_name() == Some("discussions.jsonl") && is_scoped_family(path, "threads") {
            return Self::Discussions;
        }
        if path.file_name() == Some("landings.jsonl")
            && directory.file_name() == Some("ideation")
            && directory
                .parent()
                .and_then(Utf8Path::parent)
                .is_some_and(|directory| directory.file_name() == Some("scopes"))
            && directory
                .parent()
                .and_then(Utf8Path::parent)
                .and_then(Utf8Path::parent)
                .is_some_and(|directory| directory.file_name() == Some("state"))
        {
            Self::IdeationLandings
        } else {
            Self::Unrecognized
        }
    }
}

/// Re-checks merged records against the type their shard holds, naming the
/// first record that fails.
///
/// Each recognized family is deserialized as its record type and its
/// required relations must be present. The merge handler also passes the
/// ancestor and selected records of requirement and rule shards to
/// [`changed_statement_diagnostics`] before it writes the result. Cross-record
/// checks that need the whole graph - dangling references, cycles, scope
/// membership - belong to `provenance check` and the graph validator, not
/// here: a merge driver sees one file.
pub fn validate_merged_records(
    shard_path: &Utf8Path,
    records: &[CanonicalRecord],
) -> anyhow::Result<()> {
    match ShardFamily::for_shard_path(shard_path) {
        ShardFamily::Sources => validate_typed_records::<Source>(records, "source"),
        ShardFamily::Requirements => validate_typed_records::<Requirement>(records, "requirement"),
        ShardFamily::RequirementReviews => {
            deserialize_records::<RequirementReview>(records, "requirement review").map(drop)
        }
        ShardFamily::Resolutions => validate_typed_records::<Resolution>(records, "resolution"),
        ShardFamily::Rules => validate_typed_records::<Rule>(records, "rule"),
        ShardFamily::Topics => validate_typed_records::<Topic>(records, "topic"),
        ShardFamily::Questions => validate_typed_records::<Question>(records, "question"),
        ShardFamily::IdeationLandings | ShardFamily::Discussions | ShardFamily::Unrecognized => {
            Ok(())
        }
    }
}

pub fn changed_statement_diagnostics(
    shard_path: &Utf8Path,
    base: &[CanonicalRecord],
    candidate: &[CanonicalRecord],
) -> anyhow::Result<Vec<StatementDiagnostic>> {
    match ShardFamily::for_shard_path(shard_path) {
        ShardFamily::Requirements => Ok(analyze_changed_statements(
            &deserialize_records(base, "requirement")?,
            &[],
            &deserialize_records(candidate, "requirement")?,
            &[],
            None,
        )),
        ShardFamily::Rules => Ok(analyze_changed_statements(
            &[],
            &deserialize_records(base, "rule")?,
            &[],
            &deserialize_records(candidate, "rule")?,
            None,
        )),
        _ => Ok(Vec::new()),
    }
}

fn is_scoped_family(path: &Utf8Path, family: &str) -> bool {
    let Some(family_dir) = path
        .parent()
        .filter(|path| path.file_name() == Some(family))
    else {
        return false;
    };
    family_dir
        .parent()
        .and_then(Utf8Path::parent)
        .is_some_and(|path| path.file_name() == Some("scopes"))
        && family_dir
            .parent()
            .and_then(Utf8Path::parent)
            .and_then(Utf8Path::parent)
            .is_some_and(|path| path.file_name() == Some("state"))
}

/// Every record deserializes as the family's type and carries each required
/// relation: serde builds an empty list without complaint, so the check
/// runs here.
fn validate_typed_records<T: serde::de::DeserializeOwned + RelationOwner>(
    records: &[CanonicalRecord],
    kind: &str,
) -> anyhow::Result<()> {
    for record in deserialize_records::<T>(records, kind)? {
        if let Some(decl) = missing_required(&record) {
            anyhow::bail!(
                "merged {kind} {} is refused: {}",
                record.id().as_str(),
                required_refusal(decl)
            );
        }
    }
    Ok(())
}

fn deserialize_records<T: serde::de::DeserializeOwned>(
    records: &[CanonicalRecord],
    kind: &str,
) -> anyhow::Result<Vec<T>> {
    records
        .iter()
        .map(|record| {
            let named = record
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("<record with no id>");
            serde_json::from_value(record.clone())
                .with_context(|| format!("merged record {named} is not a {kind} record"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: &str, requirement_ids: &[&str]) -> Value {
        serde_json::json!({
            "scope_id": "default",
            "id": id,
            "statement": "The merged rule shall hold.",
            "status": "active",
            "severity": "high",
            "requirement_ids": requirement_ids,
        })
    }

    fn resolution(id: &str, requirement_ids: &[&str]) -> Value {
        serde_json::json!({
            "scope_id": "default",
            "id": id,
            "title": "Merged",
            "position": "Position",
            "rationale": "Rationale",
            "status": "proposed",
            "requirement_ids": requirement_ids,
        })
    }

    fn rules_path() -> &'static Utf8Path {
        Utf8Path::new(".provenance/state/scopes/default/rules/rule.jsonl")
    }

    fn resolutions_path() -> &'static Utf8Path {
        Utf8Path::new(".provenance/state/scopes/default/resolutions/resolution.jsonl")
    }

    #[test]
    fn recognizes_every_relation_family_by_its_scoped_path() {
        for (name, family) in SCOPED_FAMILIES {
            let relative = format!(".provenance/state/scopes/default/{name}/shard.jsonl");
            assert_eq!(
                ShardFamily::for_shard_path(Utf8Path::new(&relative)),
                family
            );
            let absolute = format!("/repo/.provenance/state/scopes/default/{name}/shard.jsonl");
            assert_eq!(
                ShardFamily::for_shard_path(Utf8Path::new(&absolute)),
                family
            );
        }
    }

    #[test]
    fn recognizes_the_review_file_in_the_requirements_directory() {
        assert_eq!(
            ShardFamily::for_shard_path(Utf8Path::new(
                ".provenance/state/scopes/default/requirements/review.jsonl"
            )),
            ShardFamily::RequirementReviews
        );
        assert_eq!(
            ShardFamily::for_shard_path(Utf8Path::new(
                ".provenance/state/scopes/default/requirements/req.jsonl"
            )),
            ShardFamily::Requirements
        );
    }

    #[test]
    fn leaves_unrecognized_paths_unchecked() {
        for path in [
            "rules/rule.jsonl",
            "notes.jsonl",
            ".provenance/state/notes/notes.jsonl",
        ] {
            assert_eq!(
                ShardFamily::for_shard_path(Utf8Path::new(path)),
                ShardFamily::Unrecognized,
                "{path} should not be recognized as a typed shard"
            );
        }
        // A record that would fail the rule check passes when the path does
        // not say the file holds rules.
        validate_merged_records(Utf8Path::new("notes.jsonl"), &[rule("rule_bare", &[])]).unwrap();
    }

    #[test]
    fn accepts_merged_records_that_carry_their_required_relations() {
        validate_merged_records(rules_path(), &[rule("rule_ok", &["req_one"])]).unwrap();
        validate_merged_records(
            resolutions_path(),
            &[resolution("res_ok", &["req_one", "req_two"])],
        )
        .unwrap();
    }

    #[test]
    fn rejects_a_merged_rule_with_no_requirement() {
        let error = validate_merged_records(
            rules_path(),
            &[rule("rule_ok", &["req_one"]), rule("rule_bare", &[])],
        )
        .unwrap_err();

        let report = format!("{error:#}");
        assert!(report.contains("rule_bare"), "{report}");
        assert!(report.contains("a rule needs one requirement"), "{report}");
        assert!(!report.contains("rule_ok"), "{report}");
    }

    #[test]
    fn rejects_a_merged_resolution_with_no_requirement() {
        let error = validate_merged_records(resolutions_path(), &[resolution("res_bare", &[])])
            .unwrap_err();

        let report = format!("{error:#}");
        assert!(report.contains("res_bare"), "{report}");
        assert!(
            report.contains("a resolution needs one requirement"),
            "{report}"
        );
    }

    #[test]
    fn rejects_a_merged_record_that_is_not_its_family() {
        let error = validate_merged_records(
            rules_path(),
            &[serde_json::json!({ "id": "rule_truncated", "statement": "half" })],
        )
        .unwrap_err();

        let report = format!("{error:#}");
        assert!(report.contains("rule_truncated"), "{report}");
        assert!(report.contains("is not a rule record"), "{report}");
    }
}
