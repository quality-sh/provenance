//! Compatibility adapter for record-family consumers under migration.

macro_rules! as_legacy_rows {
    (no_argument; $consumer:ident; $($rows:tt)*) => {
        $crate::cache::family_table_legacy::as_legacy_rows!(
            @convert $consumer []; $($rows)*
        );
    };
    (argument; $consumer:ident; $argument:ident; $($rows:tt)*) => {
        $crate::cache::family_table_legacy::as_legacy_rows!(
            @convert $consumer [$argument;]; $($rows)*
        );
    };
    (
        @convert $consumer:ident [$($prefix:tt)*];
        export { $(
            $export_variant:ident {
                record: $export_type:ty, field: $export_field:ident,
                shard: { path: $export_path:ident, suffix: $export_suffix:literal, table: $export_table:literal },
                node: [$($export_node:tt)*],
                reader: { open: $export_reader:ident, closed: [$($export_closed:tt)*], strategy: $export_strategy:ident },
                id: $export_id:ident, loader: [$($export_loader:tt)*],
                graph: [$($export_graph:tt)*], import: [$($export_import:tt)*],
                catalog: [$($export_catalog:tt)*], route: [$($export_route:tt)*]
            };
        )* }
        canonical { $(
            $canonical_variant:ident {
                record: $canonical_type:ty, field: $canonical_field:ident,
                shard: { path: $canonical_path:ident, suffix: $canonical_suffix:literal, table: $canonical_table:literal },
                node: [$($canonical_node:tt)*],
                reader: { open: $canonical_reader:ident, closed: [$($canonical_closed:tt)*], strategy: $canonical_strategy:ident },
                id: $canonical_id:ident, loader: [$($canonical_loader:tt)*],
                graph: [$($canonical_graph:tt)*], import: [$($canonical_import:tt)*],
                catalog: [$($canonical_catalog:tt)*], route: [$($canonical_route:tt)*]
            };
        )* }
        bindings { $(
            $binding_variant:ident {
                record: $binding_type:ty, field: $binding_field:ident,
                shard: { path: $binding_path:ident, suffix: $binding_suffix:literal, table: $binding_table:literal },
                node: [$($binding_node:tt)*],
                reader: { open: $binding_reader:ident, closed: [$($binding_closed:tt)*], strategy: $binding_strategy:ident },
                id: $binding_id:ident, loader: [$($binding_loader:tt)*],
                graph: [$($binding_graph:tt)*], import: [$($binding_import:tt)*],
                catalog: [$($binding_catalog:tt)*], route: [$($binding_route:tt)*]
            };
        )* }
        internal { $(
            $internal_variant:ident {
                record: $internal_type:ty, field: $internal_field:ident,
                shard: { path: $internal_path:ident, suffix: $internal_suffix:literal, table: $internal_table:literal },
                node: [$($internal_node:tt)*],
                reader: { open: $internal_reader:ident, closed: [$($internal_closed:tt)*], strategy: $internal_strategy:ident },
                id: $internal_id:ident, loader: [$($internal_loader:tt)*],
                graph: [$($internal_graph:tt)*], import: [$($internal_import:tt)*],
                catalog: [$($internal_catalog:tt)*], route: [$($internal_route:tt)*]
            };
        )* }
    ) => {
        $consumer! {
            $($prefix)*
            export { $($export_variant: $export_type, $export_field, $export_path, $export_suffix, $export_table, [$($export_node)*], $export_reader, [$($export_closed)*], $export_id, [$($export_loader)*], [$($export_catalog)*];)* }
            canonical { $($canonical_variant: $canonical_type, $canonical_field, $canonical_path, $canonical_suffix, $canonical_table, [$($canonical_node)*], $canonical_reader, [$($canonical_closed)*], $canonical_id, [$($canonical_loader)*], [$($canonical_catalog)*];)* }
            bindings { $($binding_variant: $binding_type, $binding_field, $binding_path, $binding_suffix, $binding_table, [$($binding_node)*], $binding_reader, [$($binding_closed)*], $binding_id, [$($binding_loader)*], [$($binding_catalog)*];)* }
            internal { $($internal_variant: $internal_type, $internal_field, $internal_path, $internal_suffix, $internal_table, [$($internal_node)*], $internal_reader, [$($internal_closed)*], $internal_id, [$($internal_loader)*], [$($internal_catalog)*];)* }
        }
    };
}

macro_rules! record_families {
    ($consumer:ident) => {
        $crate::cache::family_table::record_family_rows!(
            $crate::cache::family_table_legacy::as_legacy_rows,
            no_argument,
            $consumer
        );
    };
    ($consumer:ident, $argument:ident) => {
        $crate::cache::family_table::record_family_rows!(
            $crate::cache::family_table_legacy::as_legacy_rows,
            argument,
            $consumer,
            $argument
        );
    };
}

pub(crate) use as_legacy_rows;
pub(crate) use record_families;
