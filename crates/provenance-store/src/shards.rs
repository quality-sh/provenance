use camino::Utf8PathBuf;
use provenance_core::{NodeType, ScopeId};

use crate::cache::{record_families, ProjectionFamily};
use crate::layout::ProvenanceLayout;

/// The canonical record file of one node kind.
pub fn path_for(layout: &ProvenanceLayout, scope: &ScopeId, kind: NodeType) -> Utf8PathBuf {
    ProjectionFamily::ALL
        .into_iter()
        .find(|family| family.node_type() == Some(kind))
        .expect("each node type has one record family")
        .shard_path(layout, scope)
}

macro_rules! define_shard_paths {
    (
        export { $($export_variant:ident: $export_type:ty, $export_field:ident, $export_path:ident, $export_suffix:literal, $export_table:literal, [$($export_node:tt)*], $export_reader:ident, [$($export_closed:tt)*], $export_id:ident, [$($export_loader:tt)*], [$($export_catalog:tt)*];)* }
        canonical { $($canonical_variant:ident: $canonical_type:ty, $canonical_field:ident, $canonical_path:ident, $canonical_suffix:literal, $canonical_table:literal, [$($canonical_node:tt)*], $canonical_reader:ident, [$($canonical_closed:tt)*], $canonical_id:ident, [$($canonical_loader:tt)*], [$($canonical_catalog:tt)*];)* }
        bindings { $($binding_variant:ident: $binding_type:ty, $binding_field:ident, $binding_path:ident, $binding_suffix:literal, $binding_table:literal, [$($binding_node:tt)*], $binding_reader:ident, [$($binding_closed:tt)*], $binding_id:ident, [$($binding_loader:tt)*], [$($binding_catalog:tt)*];)* }
        internal { $($internal_variant:ident: $internal_type:ty, $internal_field:ident, $internal_path:ident, $internal_suffix:literal, $internal_table:literal, [$($internal_node:tt)*], $internal_reader:ident, [$($internal_closed:tt)*], $internal_id:ident, [$($internal_loader:tt)*], [$($internal_catalog:tt)*];)* }
    ) => {
        $(
            pub fn $export_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
                ProjectionFamily::$export_variant.shard_path(layout, scope)
            }
        )*
        $(
            pub fn $canonical_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
                ProjectionFamily::$canonical_variant.shard_path(layout, scope)
            }
        )*
        $(
            pub fn $binding_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
                ProjectionFamily::$binding_variant.shard_path(layout, scope)
            }
        )*
        $(
            pub fn $internal_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
                ProjectionFamily::$internal_variant.shard_path(layout, scope)
            }
        )*
    };
}

record_families!(define_shard_paths);

pub(crate) fn legacy_promotion_decisions_path(
    layout: &ProvenanceLayout,
    scope: &ScopeId,
) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("ideation/promotion_decisions.jsonl")
}

pub fn ideation_landings_path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout
        .scopes_dir()
        .join(scope.as_str())
        .join("ideation/landings.jsonl")
}
