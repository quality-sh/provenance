//! Complete collection reads used by the resource routes.
use super::scoped_list::scoped_list;

macro_rules! catalog_list {
    (none, $record:ty, $reader:ident) => {};
    (projection($list:ident, $wire:literal, $($rest:tt)*), $record:ty, $reader:ident) => {
        scoped_list!($list, $wire, $record, $reader);
    };
    (payload($list:ident, $wire:literal, $($rest:tt)*), $record:ty, $reader:ident) => {
        scoped_list!($list, $wire, $record, $reader);
    };
    (verification($list:ident, $wire:literal, $($rest:tt)*), $record:ty, $reader:ident) => {
        scoped_list!($list, $wire, $record, $reader);
    };
}

macro_rules! define_record_lists {
    (
        export { $($export_variant:ident: $export_type:ty, $export_field:ident, $export_path:ident, $export_suffix:literal, $export_table:literal, [$($export_node:tt)*], $export_reader:ident, [$($export_closed:tt)*], $export_id:ident, [$($export_loader:tt)*], [$($export_catalog:tt)*];)* }
        canonical { $($canonical_variant:ident: $canonical_type:ty, $canonical_field:ident, $canonical_path:ident, $canonical_suffix:literal, $canonical_table:literal, [$($canonical_node:tt)*], $canonical_reader:ident, [$($canonical_closed:tt)*], $canonical_id:ident, [$($canonical_loader:tt)*], [$($canonical_catalog:tt)*];)* }
        internal { $($internal_variant:ident: $internal_type:ty, $internal_field:ident, $internal_path:ident, $internal_suffix:literal, $internal_table:literal, [$($internal_node:tt)*], $internal_reader:ident, [$($internal_closed:tt)*], $internal_id:ident, [$($internal_loader:tt)*], [$($internal_catalog:tt)*];)* }
    ) => {
        $(catalog_list!($($export_catalog)*, $export_type, $export_reader);)*
        $(catalog_list!($($canonical_catalog)*, $canonical_type, $canonical_reader);)*
        $(catalog_list!($($internal_catalog)*, $internal_type, $internal_reader);)*
    };
}

crate::cache::record_families!(define_record_lists);

scoped_list!(
    ListVerificationRunsV2,
    "list-verification-runs",
    VerificationRun,
    list_verification_runs
);
