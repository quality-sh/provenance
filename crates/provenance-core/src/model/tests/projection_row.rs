//! Every record kind round-trips through its projection row byte for
//! byte: `row` then `from_row` serializes to the same JSON as the record.
//!
//! Each kind has one fixture with every field filled and one with every
//! optional field empty, so both the value path and the null path of
//! every column run. A filled fixture must serialize every column name:
//! an empty field hides behind `skip_serializing_if`, and the test says
//! which one.

mod artifacts;
mod integrations;
mod shaping;

use crate::model::ProjectionRow;
use serde::Serialize;

fn assert_round_trip<K: ProjectionRow + Serialize>(record: &K) {
    let row = record.row().unwrap();
    assert_eq!(
        row.len(),
        K::COLUMNS.len(),
        "{}: one value per column",
        K::TABLE
    );
    let back = K::from_row(&row).unwrap();
    assert_eq!(
        serde_json::to_string(&back).unwrap(),
        serde_json::to_string(record).unwrap(),
        "{}: the row does not read back as the record",
        K::TABLE
    );
}

/// A filled fixture serializes every column name; a hidden field means the
/// fixture left it empty.
fn assert_every_column_serialized<K: ProjectionRow + Serialize>(record: &K) {
    let value = serde_json::to_value(record).unwrap();
    let object = value.as_object().unwrap();
    let hidden: Vec<&str> = K::COLUMNS
        .iter()
        .copied()
        .filter(|column| !object.contains_key(*column))
        .collect();
    assert!(
        hidden.is_empty(),
        "{}: the filled fixture leaves {hidden:?} empty",
        K::TABLE
    );
}

fn assert_kind_round_trips<K: ProjectionRow + Serialize>(filled: &K, bare: &K) {
    assert_every_column_serialized(filled);
    assert_round_trip(filled);
    assert_round_trip(bare);
}
