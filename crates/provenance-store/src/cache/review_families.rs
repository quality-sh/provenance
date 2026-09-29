//! Review facts generated from the canonical record-family table.
//!
//! To add a reviewable kind, add its core record type to the core review kind
//! list. Then add one export row to `family_table/rows.rs`. The row names its
//! review fields, and this module derives the kind, guarded directory, owner
//! field, and closed record deserialization from those two registries.

use provenance_core::{review::ReviewRecord, NodeType};

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

        pub fn has_enrolled_record(
            store: &crate::state_store::StateStore,
            scope: &provenance_core::ScopeId,
        ) -> anyhow::Result<bool> {
            Ok(false $(|| store.$reader(scope)?.iter().any(|record| {
                record.schema_version == provenance_core::review::REVIEW_SCHEMA_VERSION
            }))*)
        }

        pub fn review_records(
            store: &crate::state_store::StateStore,
            scope: &provenance_core::ScopeId,
        ) -> anyhow::Result<Vec<ReviewRecord>> {
            let mut records = Vec::new();
            $(records.extend(store.$reader(scope)?.into_iter().map(ReviewRecord::from));)*
            Ok(records)
        }

        pub fn review_paths(
            layout: &crate::layout::ProvenanceLayout,
            scope: &provenance_core::ScopeId,
        ) -> Vec<camino::Utf8PathBuf> {
            vec![$(crate::shards::$path(layout, scope),)*]
        }
    };
}

crate::cache::family_table::record_family_rows!(define_review_families);

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
            assert!(!facts.content_fields.is_empty());
            assert!(!facts.lifecycle_fields.is_empty());
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
}
