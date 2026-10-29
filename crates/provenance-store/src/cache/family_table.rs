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

/// Defines `record_family_catalog!`, the only view of the family rows that other crates can
/// expand. Each row keeps its variant, record type, route order, node kind, operations,
/// routes and review marker. Shards, readers, loaders, graph fields and import rules stay in
/// this crate.
macro_rules! define_record_family_catalog {
    (
        $d:tt;
        $($group:ident {
            $($variant:ident {
                record: $record:ty,
                field: $field:ident,
                path: $path:ident,
                meta: {
                    shard: $shard:literal,
                    order: $order:tt,
                    budget: $budget:ident
                    $(, terminal: [$($terminal:literal),*])?
                },
                node: [$($node:tt)*],
                reader: {
                    open: $reader:ident,
                    closed: [$($closed:tt)*],
                    strategy: $strategy:ident
                },
                id: $id:ident,
                loader: [$($loader:tt)*],
                graph: [$($graph:tt)*],
                import: [$($import:tt)*],
                catalog: [$($catalog:tt)*],
                route: [$($route:tt)*]
                $(, review: $review:ident)?
            };)*
        })*
    ) => {
        /// Expands `$consumer!` with one narrow row for each canonical record family:
        /// `variant { record, order, node, catalog, route, review? }`. An optional
        /// identifier is passed first, followed by `;`.
        #[macro_export]
        macro_rules! record_family_catalog {
            ($d consumer:path) => {
                $d consumer! {
                    $($group {
                        $($variant {
                            record: $record,
                            order: $order,
                            node: [$($node)*],
                            catalog: [$($catalog)*],
                            route: [$($route)*]
                            $(, review: $review)?
                        };)*
                    })*
                }
            };
            ($d consumer:path, $d argument:ident) => {
                $d consumer! {
                    $d argument;
                    $($group {
                        $($variant {
                            record: $record,
                            order: $order,
                            node: [$($node)*],
                            catalog: [$($catalog)*],
                            route: [$($route)*]
                            $(, review: $review)?
                        };)*
                    })*
                }
            };
        }
    };
}

expand_record_family_rows!((define_record_family_catalog) [$;]);
