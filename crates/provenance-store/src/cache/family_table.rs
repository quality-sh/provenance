//! Canonical record-family types and named fields.
//!
//! A new canonical record kind must have one row in `family_table/rows.rs`.

mod rows;
pub(crate) use rows::expand_record_family_rows;

macro_rules! record_family_rows {
    ($consumer:path) => {
        $crate::cache::family_table::expand_record_family_rows!(($consumer) []);
    };
    ($consumer:path, $argument:ident) => {
        $crate::cache::family_table::expand_record_family_rows!(($consumer) [$argument;]);
    };
    ($consumer:path, $first:ident, $second:ident) => {
        $crate::cache::family_table::expand_record_family_rows!(
            ($consumer) [$first; $second;]
        );
    };
    ($consumer:path, $first:ident, $second:ident, $third:ident) => {
        $crate::cache::family_table::expand_record_family_rows!(
            ($consumer) [$first; $second; $third;]
        );
    };
}

pub(crate) use record_family_rows;
