//! Complete collection reads used by the resource routes.
use super::scoped_list::scoped_list;

macro_rules! catalog_list {
    (none, $record:ty, $reader:ident) => {};
    (projection($list:ident, $wire:literal, $($rest:tt)*), $record:ty, $reader:ident) => {
        scoped_list!($list, $wire, type $record, $reader);
    };
    (payload($list:ident, $wire:literal, $($rest:tt)*), $record:ty, $reader:ident) => {
        scoped_list!($list, $wire, type $record, $reader);
    };
    (verification($list:ident, $wire:literal, $($rest:tt)*), $record:ty, $reader:ident) => {
        scoped_list!($list, $wire, type $record, $reader);
    };
}

macro_rules! define_record_lists {
    (
        $(
            $group:ident {
                $(
                    $variant:ident {
                        record: $record:ty,
                        field: $field:ident,
                        path: $path:ident,
                        node: [$($node:tt)*],
                        reader: $reader:ident,
                        closed: [$($closed:tt)*],
                        strategy: $strategy:ident,
                        id: $id:ident,
                        loader: [$($loader:tt)*],
                        catalog: [$($catalog:tt)*]
                    };
                )*
            }
        )*
    ) => {
        $($(catalog_list!($($catalog)*, $record, $reader);)*)*
    };
}

crate::cache::record_families!(define_record_lists);

scoped_list!(
    ListVerificationRuns,
    "list-verification-runs",
    VerificationRun,
    list_verification_runs
);
