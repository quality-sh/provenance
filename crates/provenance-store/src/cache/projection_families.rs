use crate::layout::ProvenanceLayout;
use crate::state_store::{GuardedStore, StateStore};
use camino::Utf8PathBuf;
use provenance_core::ScopeId;

pub trait RecordFamily: serde::Serialize {
    const META: &'static FamilyMeta;

    fn open(store: &StateStore, scope: &ScopeId) -> anyhow::Result<Vec<Self>>
    where
        Self: Sized;

    fn guarded(store: &GuardedStore<'_>, scope: &ScopeId) -> anyhow::Result<Vec<Self>>
    where
        Self: Sized;

    fn record_id(&self) -> &str;
}

macro_rules! define_projection_families {
    (
        $(
            $group:ident {
                $(
                    $variant:ident {
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
                    };
                )*
            }
        )*
    ) => {
        /// One family of canonical records stored in the projection.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ProjectionFamily {
            $($($variant,)*)*
        }

        $($(impl RecordFamily for $record {
            const META: &'static FamilyMeta = &FamilyMeta {
                group: family_group!($group),
                table_name: stringify!($field),
                shard_suffix: $shard,
                node_type: family_node!([$($node)*]),
                graph_field: family_graph_field!($group, $field),
                route_order: family_order!($order),
                budget: BudgetKind::$budget,
                terminal_statuses: terminal_statuses!($([$($terminal),*])?),
                catalog_operations: family_catalog!($($catalog)*),
            };

            fn open(store: &StateStore, scope: &ScopeId) -> anyhow::Result<Vec<Self>> {
                store.$reader(scope)
            }

            fn guarded(
                store: &GuardedStore<'_>,
                scope: &ScopeId,
            ) -> anyhow::Result<Vec<Self>> {
                store.$reader(scope)
            }

            fn record_id(&self) -> &str {
                family_record_id!(self, $id)
            }
        })*)*

        impl ProjectionFamily {
            pub const ALL: [Self; FAMILIES.len()] = [
                $($(Self::$variant,)*)*
            ];

            pub const fn family_name(self) -> &'static str {
                self.meta().table_name
            }

            pub const fn shard_suffix(self) -> &'static str {
                self.meta().shard_suffix
            }

            pub const fn graph_field(self) -> Option<&'static str> {
                match self.meta().group {
                    FamilyGroup::Export | FamilyGroup::Binding => self.meta().graph_field,
                    FamilyGroup::Canonical | FamilyGroup::Internal => None,
                }
            }

            #[cfg(test)]
            pub(crate) const fn catalog_operation_names(self) -> &'static [&'static str] {
                self.meta().catalog_operations
            }

            pub(crate) const fn node_type(self) -> Option<provenance_core::NodeType> {
                self.meta().node_type
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
                    $($(Self::$variant => {
                        sorted_bytes(<$record as RecordFamily>::open(store, scope)?)
                    },)*)*
                }
            }

            pub(crate) fn guarded_records(
                self,
                store: &GuardedStore<'_>,
                scope: &ScopeId,
            ) -> anyhow::Result<(Vec<u8>, u64)> {
                match self {
                    $($(Self::$variant => {
                        sorted_bytes(<$record as RecordFamily>::guarded(store, scope)?)
                    },)*)*
                }
            }

            pub(crate) const fn meta(self) -> &'static FamilyMeta {
                match self {
                    $($(Self::$variant => <$record as RecordFamily>::META,)*)*
                }
            }
        }

        pub const FAMILIES: &[FamilyMeta] = &[
            $($(*<$record as RecordFamily>::META,)*)*
        ];
    };
}

macro_rules! family_record_id {
    ($record:expr, field) => {
        $record.id.as_str()
    };
    ($record:expr, method) => {
        $record.id().as_str()
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FamilyGroup {
    Export,
    Canonical,
    Binding,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetKind {
    Resource,
    Record,
    Unchecked,
    NotImported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamilyMeta {
    pub group: FamilyGroup,
    pub table_name: &'static str,
    pub shard_suffix: &'static str,
    pub node_type: Option<provenance_core::NodeType>,
    pub graph_field: Option<&'static str>,
    pub route_order: Option<u16>,
    pub budget: BudgetKind,
    pub terminal_statuses: &'static [&'static str],
    #[allow(dead_code)]
    pub catalog_operations: &'static [&'static str],
}

macro_rules! terminal_statuses {
    () => {
        &[]
    };
    ([$($status:literal),*]) => {
        &[$($status),*]
    };
}

/// Returns the SQL predicate for terminal-and-dead record identities.
pub(super) fn terminal_and_dead_predicate(kind: &str, id: &str, scope: &str) -> String {
    FAMILIES
        .iter()
        .filter(|family| !family.terminal_statuses.is_empty())
        .filter_map(|family| {
            let node_type = family.node_type?;
            let statuses = family
                .terminal_statuses
                .iter()
                .map(|status| format!("'{status}'"))
                .collect::<Vec<_>>()
                .join(", ");
            Some(format!(
                "({kind} = '{}' AND EXISTS (SELECT 1 FROM {} lifecycle \
                 WHERE lifecycle.scope_id = {scope} AND lifecycle.id = {id} \
                 AND lifecycle.status IN ({statuses})))",
                node_type.as_str(),
                family.table_name,
            ))
        })
        .collect::<Vec<_>>()
        .join(" OR ")
}

macro_rules! family_group {
    (export) => {
        FamilyGroup::Export
    };
    (canonical) => {
        FamilyGroup::Canonical
    };
    (bindings) => {
        FamilyGroup::Binding
    };
    (internal) => {
        FamilyGroup::Internal
    };
}

macro_rules! family_node {
    ([]) => {
        None
    };
    ([$node:ident]) => {
        Some(provenance_core::NodeType::$node)
    };
}

macro_rules! family_graph_field {
    (export, $field:ident) => {
        Some(stringify!($field))
    };
    (bindings, $field:ident) => {
        Some(stringify!($field))
    };
    ($group:ident, $field:ident) => {
        None
    };
}

macro_rules! family_order {
    (none) => {
        None
    };
    ($order:literal) => {
        Some($order)
    };
}

macro_rules! family_catalog {
    (none) => {
        &[]
    };
    (
        projection(
            $list:ident, $list_wire:literal, $page:ident, $page_wire:literal, none
        )
    ) => {
        &[$list_wire, $page_wire]
    };
    (
        $kind:ident(
            $list:ident,
            $list_wire:literal,
            $page:ident,
            $page_wire:literal,
            $member:ident,
            $member_wire:literal
        )
    ) => {
        &[$list_wire, $page_wire, $member_wire]
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

fn sorted_bytes<T: RecordFamily>(mut records: Vec<T>) -> anyhow::Result<(Vec<u8>, u64)> {
    records.sort_by(|left, right| left.record_id().cmp(right.record_id()));
    let count = records.len() as u64;
    Ok((crate::canonical_digest::canonical_bytes(&records)?, count))
}

#[cfg(test)]
mod tests;
