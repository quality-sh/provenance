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
        $(
            $group:ident {
                $(
                    $variant:ident {
                        record: $record:ty,
                        field: $field:ident,
                        path: $path:ident,
                        $($rest:tt)*
                    };
                )*
            }
        )*
    ) => {
        $(
            $(
                pub fn $path(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
                    ProjectionFamily::$variant.shard_path(layout, scope)
                }
            )*
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
