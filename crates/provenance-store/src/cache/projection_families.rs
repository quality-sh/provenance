use crate::layout::ProvenanceLayout;
use crate::state_store::{GuardedStore, StateStore};
use camino::Utf8PathBuf;
use provenance_core::ScopeId;

macro_rules! define_projection_families {
    (
        export { $(
            $export_variant:ident { record: $export_type:ty, field: $export_field:ident,
            shard: { path: $export_path:ident, suffix: $export_suffix:literal, table: $export_table:literal },
            node: [$($export_node:tt)*], reader: { open: $export_reader:ident, closed: [$($export_closed:tt)*], strategy: $export_strategy:ident },
            id: $export_id:ident, loader: [$($export_loader:tt)*], graph: [$($export_graph:tt)*], import: [$($export_import:tt)*],
            catalog: [$($export_catalog:tt)*], route: [$($export_route:tt)*] };)* }
        canonical { $(
            $canonical_variant:ident { record: $canonical_type:ty, field: $canonical_field:ident,
            shard: { path: $canonical_path:ident, suffix: $canonical_suffix:literal, table: $canonical_table:literal },
            node: [$($canonical_node:tt)*], reader: { open: $canonical_reader:ident, closed: [$($canonical_closed:tt)*], strategy: $canonical_strategy:ident },
            id: $canonical_id:ident, loader: [$($canonical_loader:tt)*], graph: [$($canonical_graph:tt)*], import: [$($canonical_import:tt)*],
            catalog: [$($canonical_catalog:tt)*], route: [$($canonical_route:tt)*] };)* }
        bindings { $(
            $binding_variant:ident { record: $binding_type:ty, field: $binding_field:ident,
            shard: { path: $binding_path:ident, suffix: $binding_suffix:literal, table: $binding_table:literal },
            node: [$($binding_node:tt)*], reader: { open: $binding_reader:ident, closed: [$($binding_closed:tt)*], strategy: $binding_strategy:ident },
            id: $binding_id:ident, loader: [$($binding_loader:tt)*], graph: [$($binding_graph:tt)*], import: [$($binding_import:tt)*],
            catalog: [$($binding_catalog:tt)*], route: [$($binding_route:tt)*] };)* }
        internal { $(
            $internal_variant:ident { record: $internal_type:ty, field: $internal_field:ident,
            shard: { path: $internal_path:ident, suffix: $internal_suffix:literal, table: $internal_table:literal },
            node: [$($internal_node:tt)*], reader: { open: $internal_reader:ident, closed: [$($internal_closed:tt)*], strategy: $internal_strategy:ident },
            id: $internal_id:ident, loader: [$($internal_loader:tt)*], graph: [$($internal_graph:tt)*], import: [$($internal_import:tt)*],
            catalog: [$($internal_catalog:tt)*], route: [$($internal_route:tt)*] };)* }
    ) => {
        /// One family of canonical records stored in the projection.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ProjectionFamily {
            $($export_variant,)*
            $($canonical_variant,)*
            $($binding_variant,)*
            $($internal_variant,)*
        }

        impl ProjectionFamily {
            pub const ALL: [Self; count_families!($($export_variant)* $($canonical_variant)* $($binding_variant)* $($internal_variant)*)] = [
                $(Self::$export_variant,)*
                $(Self::$canonical_variant,)*
                $(Self::$binding_variant,)*
                $(Self::$internal_variant,)*
            ];

            pub const fn family_name(self) -> &'static str {
                match self {
                    $(Self::$export_variant => $export_table,)*
                    $(Self::$canonical_variant => $canonical_table,)*
                    $(Self::$binding_variant => $binding_table,)*
                    $(Self::$internal_variant => $internal_table,)*
                }
            }

            pub const fn shard_suffix(self) -> &'static str {
                match self {
                    $(Self::$export_variant => $export_suffix,)*
                    $(Self::$canonical_variant => $canonical_suffix,)*
                    $(Self::$binding_variant => $binding_suffix,)*
                    $(Self::$internal_variant => $internal_suffix,)*
                }
            }

            pub const fn graph_field(self) -> Option<&'static str> {
                match self {
                    $(Self::$export_variant => Some(stringify!($export_field)),)*
                    $(Self::$binding_variant => Some(stringify!($binding_field)),)*
                    _ => None,
                }
            }

            #[cfg(test)]
            pub(crate) const fn catalog_operation_names(self) -> &'static [&'static str] {
                match self {
                    $(Self::$export_variant => catalog_names!($($export_catalog)*),)*
                    $(Self::$canonical_variant => catalog_names!($($canonical_catalog)*),)*
                    $(Self::$binding_variant => catalog_names!($($binding_catalog)*),)*
                    $(Self::$internal_variant => catalog_names!($($internal_catalog)*),)*
                }
            }

            pub(crate) const fn node_type(self) -> Option<provenance_core::NodeType> {
                match self {
                    $(Self::$export_variant => family_node_type!($($export_node)*),)*
                    $(Self::$canonical_variant => family_node_type!($($canonical_node)*),)*
                    $(Self::$binding_variant => family_node_type!($($binding_node)*),)*
                    $(Self::$internal_variant => family_node_type!($($internal_node)*),)*
                }
            }

            pub(crate) fn shard_path(
                self,
                layout: &ProvenanceLayout,
                scope: &ScopeId,
            ) -> Utf8PathBuf {
                layout.scopes_dir().join(scope.as_str()).join(self.shard_suffix())
            }

            pub(crate) fn canonical_records(
                self,
                store: &StateStore,
                scope: &ScopeId,
            ) -> anyhow::Result<(Vec<u8>, u64)> {
                match self {
                    $(Self::$export_variant => sorted_bytes(store.$export_reader(scope)?, family_id!($export_id)),)*
                    $(Self::$canonical_variant => sorted_bytes(store.$canonical_reader(scope)?, family_id!($canonical_id)),)*
                    $(Self::$binding_variant => sorted_bytes(store.$binding_reader(scope)?, family_id!($binding_id)),)*
                    $(Self::$internal_variant => sorted_bytes(store.$internal_reader(scope)?, family_id!($internal_id)),)*
                }
            }

            pub(crate) fn guarded_records(
                self,
                store: &GuardedStore<'_>,
                scope: &ScopeId,
            ) -> anyhow::Result<(Vec<u8>, u64)> {
                match self {
                    $(Self::$export_variant => sorted_bytes(store.$export_reader(scope)?, family_id!($export_id)),)*
                    $(Self::$canonical_variant => sorted_bytes(store.$canonical_reader(scope)?, family_id!($canonical_id)),)*
                    $(Self::$binding_variant => sorted_bytes(store.$binding_reader(scope)?, family_id!($binding_id)),)*
                    $(Self::$internal_variant => sorted_bytes(store.$internal_reader(scope)?, family_id!($internal_id)),)*
                }
            }
        }
    };
}

macro_rules! count_families {
    () => { 0usize };
    ($family:ident $($rest:ident)*) => { 1usize + count_families!($($rest)*) };
}

#[cfg(test)]
macro_rules! catalog_names {
    (none) => {
        &[]
    };
    ($kind:ident($list:ident, $list_wire:literal, $page:ident, $page_wire:literal, none)) => {
        &[$list_wire, $page_wire]
    };
    ($kind:ident($list:ident, $list_wire:literal, $page:ident, $page_wire:literal, $member:ident, $member_wire:literal)) => {
        &[$list_wire, $page_wire, $member_wire]
    };
    (verification($list:ident, $list_wire:literal, $page:ident, $member:ident, $member_wire:literal)) => {
        &[$list_wire, "page-verification-bindings-v2", $member_wire]
    };
}

macro_rules! family_node_type {
    () => {
        None
    };
    ($node:ident) => {
        Some(provenance_core::NodeType::$node)
    };
}

macro_rules! family_id {
    (field) => {
        |record| record.id.as_str()
    };
    (method) => {
        |record| record.id().as_str()
    };
}

crate::cache::family_table::record_family_rows!(define_projection_families);

impl ProjectionFamily {
    pub(crate) fn content_digest(self, bytes: &[u8]) -> anyhow::Result<String> {
        if !matches!(
            self,
            Self::Sources | Self::Requirements | Self::Rules | Self::Resolutions
        ) {
            return Ok(crate::canonical_digest::digest(bytes));
        }
        let mut records: Vec<serde_json::Value> = serde_json::from_slice(bytes)?;
        for record in &mut records {
            if let Some(record) = record.as_object_mut() {
                record.remove("created");
                record.remove("updated");
            }
        }
        Ok(crate::canonical_digest::digest(
            &crate::canonical_digest::canonical_bytes(&records)?,
        ))
    }
}

fn sorted_bytes<T: serde::Serialize>(
    mut records: Vec<T>,
    id: impl Fn(&T) -> &str,
) -> anyhow::Result<(Vec<u8>, u64)> {
    records.sort_by(|left, right| id(left).cmp(id(right)));
    let count = records.len() as u64;
    Ok((crate::canonical_digest::canonical_bytes(&records)?, count))
}

#[cfg(test)]
mod tests {
    use super::{ProjectionFamily, FAMILIES};

    #[test]
    fn every_projection_family_has_one_plain_metadata_row() {
        assert_eq!(ProjectionFamily::ALL.len(), FAMILIES.len());
        for (family, metadata) in ProjectionFamily::ALL.into_iter().zip(FAMILIES) {
            assert_eq!(family, metadata.family);
            assert_eq!(family.family_name(), metadata.table_name);
            assert_eq!(family.shard_suffix(), metadata.shard_suffix);
            assert_eq!(family.node_type(), metadata.node_type);
        }
    }

    #[test]
    fn family_metadata_keeps_stable_storage_and_export_names_together() {
        assert_eq!(ProjectionFamily::Sources.family_name(), "sources");
        assert_eq!(
            ProjectionFamily::Sources.shard_suffix(),
            "sources/source.jsonl"
        );
        assert_eq!(ProjectionFamily::Sources.graph_field(), Some("sources"));
        assert_eq!(
            ProjectionFamily::SynthesisPackets.family_name(),
            "synthesis_packets"
        );
        assert_eq!(
            ProjectionFamily::SynthesisPackets.shard_suffix(),
            "ideation/synthesis_packets.jsonl"
        );
        assert_eq!(ProjectionFamily::SynthesisPackets.graph_field(), None);
    }

    #[test]
    fn catalog_registers_each_family_operation() {
        let registered = crate::operations::catalog::registered_operation_names_for_test();
        for family in ProjectionFamily::ALL {
            for operation in family.catalog_operation_names() {
                assert!(
                    registered.contains(operation),
                    "{} operation {operation} is absent from the catalog",
                    family.family_name()
                );
            }
        }
    }
}
