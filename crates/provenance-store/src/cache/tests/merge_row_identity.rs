//! The merge driver keys each state file of a scope by the identity that the
//! rows of that file have.

use super::projection_digest_sensitivity::{aggregate_layout, change_one_record};
use crate::families::ProjectionFamily;
use crate::merge::{BatchRecords, RowIdentity, ShardFamily};
use crate::state_store::IdeationLandingBatch;
use camino::{Utf8Path, Utf8PathBuf};
use serde_json::Value;
use std::collections::BTreeSet;

fn rows(path: &Utf8Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// A scope where each state file holds a row, and the path of each file: each
/// fixed shard of a scope, with each month file of messages in place of the
/// fixed month name.
fn state_files() -> (tempfile::TempDir, Vec<Utf8PathBuf>) {
    let (dir, layout, scope) = aggregate_layout();
    for family in ProjectionFamily::ALL {
        if rows(&family.shard_path(&layout, &scope)).is_empty() {
            change_one_record(&layout, family, &scope);
        }
    }
    let batch: IdeationLandingBatch = serde_json::from_value(serde_json::json!({
        "contributions": [{
            "scope_id": scope.as_str(), "id": "contribution_landed",
            "target": {"artifact_type": "requirement", "artifact_id": "req_schads_overtime"},
            "participant_slot": "slot_a", "stance": "support",
            "strongest_finding": "Landed", "evidence_references": [], "material_claims": [],
            "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
            "unsupported_recommendations": [],
            "uncertainty": {"level": "low", "rationale": "R"}, "open_questions": []
        }]
    }))
    .unwrap();
    let store = crate::test_support::store(&layout);
    store.land_ideation_batch(&scope, batch, false).unwrap();
    for family in ProjectionFamily::ALL {
        family.canonical_records(&store, &scope).unwrap();
    }
    let landings = crate::shards::ideation_landings_path(&layout, &scope);
    let scope_dir = landings.parent().unwrap().parent().unwrap().to_owned();
    let mut files: Vec<Utf8PathBuf> = crate::shards::shard_suffixes()
        .filter(|suffix| {
            !suffix
                .strip_prefix("threads/")
                .is_some_and(crate::shards::is_message_month_shard_name)
        })
        .map(|suffix| scope_dir.join(suffix))
        .collect();
    let months = scope_dir
        .join("threads")
        .read_dir_utf8()
        .unwrap()
        .map(|entry| entry.unwrap().path().to_owned())
        .filter(|path| crate::shards::is_message_month_shard_name(path.file_name().unwrap()))
        .collect::<Vec<_>>();
    assert!(!months.is_empty(), "the fixture writes no month file of messages");
    files.extend(months);
    (dir, files)
}

/// Each row of a file has a string in the field of the identity, and no two
/// rows have the same string.
fn assert_field_identity(path: &Utf8Path, field: &str, rows: &[Value]) {
    let mut keys = BTreeSet::new();
    for row in rows {
        let key = row.get(field).and_then(Value::as_str);
        assert!(key.is_some(), "{path}: a row has no string {field}: {row}");
        assert!(keys.insert(key), "{path}: two rows have the {field} {key:?}");
    }
}

/// Each row of a file is a batch with the lists that the identity names, and
/// each record of a list has a string in the key field.
fn assert_batch_identity(path: &Utf8Path, records: BatchRecords, rows: &[Value]) {
    let empty = IdeationLandingBatch {
        contributions: Vec::new(),
        synthesis_packets: Vec::new(),
        proposals: Vec::new(),
        assertions: Vec::new(),
        dispositions: Vec::new(),
    };
    let written = serde_json::to_value(empty).unwrap();
    let fields: BTreeSet<&str> = written.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(fields, records.lists().iter().copied().collect());
    for row in rows {
        for list in records.lists() {
            for record in row[list].as_array().unwrap() {
                assert!(
                    record.get(records.key()).and_then(Value::as_str).is_some(),
                    "{path}: a record of {list} has no string {}: {record}",
                    records.key()
                );
            }
        }
    }
}

// No Rule binding: an aid to the implementation. A state file whose rows do
// not have the identity that the driver selects for its path stops this test,
// and a fixed shard that the fixture does not write stops it too.
#[test]
fn each_state_file_of_a_scope_has_the_row_identity_of_the_merge_driver() {
    let (_dir, files) = state_files();
    for path in files {
        let rows = rows(&path);
        assert!(!rows.is_empty(), "{path}: the fixture writes no row");
        match ShardFamily::for_shard_path(&path).row_identity() {
            RowIdentity::Field(field) => assert_field_identity(&path, field, &rows),
            RowIdentity::Batch(records) => assert_batch_identity(&path, records, &rows),
        }
    }
}
