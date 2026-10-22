//! Review facts generated from the canonical record-family table.
//!
//! To add a reviewable kind, add its core record type to the core review kind
//! list. Then add one export row to `family_table/rows.rs`. The row names its
//! review fields, and this module derives the kind, guarded directory, owner
//! field, and closed record deserialization from those two registries.

use crate::state_store::StateStore;
use provenance_core::{
    protocol::read_failure::ReadFailure, review::ReviewRecord, NodeType, ScopeId, StableId,
};
use std::collections::{btree_map::Entry, BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy)]
pub struct ReviewFamily {
    pub kind: NodeType,
    pub directory: &'static str,
    pub owner_field: &'static str,
    pub content_fields: &'static [&'static str],
    pub lifecycle_fields: &'static [&'static str],
}

macro_rules! define_review_families {
    (
        export { $(
            $variant:ident {
                record: $record:ty,
                field: $field:ident,
                path: $path:ident,
                meta: $meta:tt,
                node: [$kind:ident],
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
                route: [$($route:tt)*],
                review: $review:ident
            };
        )* }
        $($other:tt)*
    ) => {
        pub const REVIEW_FAMILIES: &[ReviewFamily] = &[
            $(ReviewFamily {
                kind: NodeType::$kind,
                directory: stringify!($field),
                owner_field: "id",
                content_fields: super::review_facts::$review.content_fields,
                lifecycle_fields: super::review_facts::$review.lifecycle_fields,
            },)*
        ];

        pub fn review_records(
            store: &StateStore,
            scope: &ScopeId,
        ) -> anyhow::Result<Vec<ReviewRecord>> {
            let mut records = Vec::new();
            for family in REVIEW_FAMILIES {
                records.extend(records_of_kind(store, scope, family.kind)?);
            }
            Ok(records)
        }

        pub fn review_paths(
            layout: &crate::layout::ProvenanceLayout,
            scope: &ScopeId,
        ) -> Vec<camino::Utf8PathBuf> {
            vec![$(crate::shards::$path(layout, scope),)*]
        }

        pub fn records_of_kind(
            store: &StateStore,
            scope: &ScopeId,
            kind: NodeType,
        ) -> anyhow::Result<Vec<ReviewRecord>> {
            match kind {
                $(NodeType::$kind => Ok(store.$reader(scope)?
                    .into_iter().map(ReviewRecord::from).collect()),)*
            }
        }

        pub fn record(
            store: &StateStore,
            scope: &ScopeId,
            kind: NodeType,
            id: &StableId,
        ) -> anyhow::Result<ReviewRecord> {
            records_of_kind(store, scope, kind)?
                .into_iter()
                .find(|record| record.id() == id)
                .ok_or_else(|| ReadFailure::ResourceNotFound.into())
        }
    };
}

crate::cache::family_table::record_family_rows!(define_review_families);

/// Loads each family at first use, in caller order, and keeps its first record per id.
/// The index belongs to one read under the repository publication lock.
pub(crate) struct ReviewFamilyIndex<'a> {
    store: &'a StateStore,
    scope: &'a ScopeId,
    wanted: BTreeMap<u8, BTreeSet<&'a str>>,
    // NodeType and StableId have neither Ord nor Hash; rank and text preserve their identity.
    families: BTreeMap<u8, BTreeMap<String, ReviewRecord>>,
}

impl<'a> ReviewFamilyIndex<'a> {
    pub(crate) fn new(
        store: &'a StateStore,
        scope: &'a ScopeId,
        records: &'a [(NodeType, StableId)],
    ) -> Self {
        let mut wanted = BTreeMap::<u8, BTreeSet<&str>>::new();
        for (kind, id) in records {
            wanted.entry(kind.rank()).or_default().insert(id.as_str());
        }
        Self {
            store,
            scope,
            wanted,
            families: BTreeMap::new(),
        }
    }

    pub(crate) fn get(&mut self, kind: NodeType, id: &StableId) -> anyhow::Result<&ReviewRecord> {
        let family = match self.families.entry(kind.rank()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let mut indexed = BTreeMap::new();
                for record in records_of_kind(self.store, self.scope, kind)? {
                    if self
                        .wanted
                        .get(&kind.rank())
                        .is_some_and(|ids| ids.contains(record.id().as_str()))
                    {
                        indexed
                            .entry(record.id().as_str().to_owned())
                            .or_insert(record);
                    }
                }
                entry.insert(indexed)
            }
        };
        family
            .get(id.as_str())
            .ok_or_else(|| ReadFailure::ResourceNotFound.into())
    }
}

pub fn by_kind(kind: NodeType) -> &'static ReviewFamily {
    REVIEW_FAMILIES
        .iter()
        .find(|family| family.kind == kind)
        .expect("all NodeType values have review-family facts")
}

pub fn by_directory(directory: &str) -> Option<&'static ReviewFamily> {
    REVIEW_FAMILIES
        .iter()
        .find(|family| family.directory == directory)
}

pub fn deserialize_record(
    kind: NodeType,
    value: &serde_json::Value,
) -> anyhow::Result<ReviewRecord> {
    ReviewRecord::deserialize_closed(kind, value)
}

#[cfg(test)]
mod tests {
    use super::{by_kind, REVIEW_FAMILIES};
    use provenance_core::NodeType;

    #[test]
    fn every_record_kind_has_one_review_family() {
        assert_eq!(REVIEW_FAMILIES.len(), NodeType::ALL.len());
        for kind in NodeType::ALL {
            let facts = by_kind(kind);
            assert_eq!(facts.kind, kind);
            assert_eq!(facts.owner_field, "id");
            assert_ne!(facts.content_fields, [] as [&str; 0]);
            assert_ne!(facts.lifecycle_fields, [] as [&str; 0]);
        }
    }

    #[test]
    fn desired_inventory_contains_every_core_review_kind() {
        let canonical = provenance_core::review::REVIEW_RECORD_KINDS;
        assert_eq!(REVIEW_FAMILIES.len(), canonical.len());
        for kind in canonical {
            assert!(REVIEW_FAMILIES.iter().any(|family| family.kind == *kind));
        }
    }

    #[test]
    fn common_audit_fields_are_lifecycle_fields_when_present() {
        let common = [
            "schema_version",
            "scope_id",
            "id",
            "created",
            "updated",
            "origin_thread",
            "origin_message",
        ];
        for kind in [
            NodeType::Source,
            NodeType::Requirement,
            NodeType::Resolution,
            NodeType::Rule,
        ] {
            let family = by_kind(kind);
            for field in common {
                assert!(family.lifecycle_fields.contains(&field));
            }
        }
    }

    #[test]
    fn every_review_record_type_maps_to_its_canonical_kind() {
        fn kind<T: provenance_core::review::ReviewRecordKind>() -> NodeType {
            T::KIND
        }
        assert_eq!(kind::<provenance_core::Source>(), NodeType::Source);
        assert_eq!(
            kind::<provenance_core::Requirement>(),
            NodeType::Requirement
        );
        assert_eq!(kind::<provenance_core::Resolution>(), NodeType::Resolution);
        assert_eq!(kind::<provenance_core::Rule>(), NodeType::Rule);
        assert_eq!(kind::<provenance_core::Domain>(), NodeType::Domain);
        assert_eq!(kind::<provenance_core::Boundary>(), NodeType::Boundary);
        assert_eq!(kind::<provenance_core::Topic>(), NodeType::Topic);
        assert_eq!(kind::<provenance_core::Question>(), NodeType::Question);
    }
}
