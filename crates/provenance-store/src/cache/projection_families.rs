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
                self.meta().table_name
            }

            pub const fn shard_suffix(self) -> &'static str {
                self.meta().shard_suffix
            }

            pub const fn graph_field(self) -> Option<&'static str> {
                self.meta().graph_field
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

macro_rules! family_id {
    (field) => {
        |record| record.id.as_str()
    };
    (method) => {
        |record| record.id().as_str()
    };
}

crate::cache::family_table::record_family_rows!(define_projection_families);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FamilyGroup {
    Export,
    Canonical,
    Binding,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetKind {
    Resource,
    Record,
    Unchecked,
    NotImported,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FamilyMeta {
    pub family: ProjectionFamily,
    pub group: FamilyGroup,
    pub table_name: &'static str,
    pub shard_suffix: &'static str,
    pub node_type: Option<provenance_core::NodeType>,
    pub graph_field: Option<&'static str>,
    pub route_order: Option<u16>,
    pub budget: BudgetKind,
    pub catalog_operations: &'static [&'static str],
}

use BudgetKind::{NotImported, Record, Resource, Unchecked};
use FamilyGroup::{Binding, Canonical, Export, Internal};
use ProjectionFamily::*;

pub(crate) const FAMILIES: &[FamilyMeta] = &[
    family(
        Sources,
        Export,
        "sources",
        "sources/source.jsonl",
        Some(provenance_core::NodeType::Source),
        Some("sources"),
        Some(10),
        Resource,
        &["list-sources", "page-sources-v2", "get-source-v2"],
    ),
    family(
        Domains,
        Export,
        "domains",
        "domains/domain.jsonl",
        Some(provenance_core::NodeType::Domain),
        Some("domains"),
        Some(50),
        Resource,
        &["list-domains", "page-domains-v2", "get-domain-v2"],
    ),
    family(
        Requirements,
        Export,
        "requirements",
        "requirements/req.jsonl",
        Some(provenance_core::NodeType::Requirement),
        Some("requirements"),
        Some(20),
        Resource,
        &["list-requirements", "page-requirements-v2"],
    ),
    family(
        Boundaries,
        Export,
        "boundaries",
        "boundaries/boundary.jsonl",
        Some(provenance_core::NodeType::Boundary),
        Some("boundaries"),
        Some(60),
        Resource,
        &["list-boundaries", "page-boundaries-v2", "get-boundary-v2"],
    ),
    family(
        Topics,
        Export,
        "topics",
        "topics/topic.jsonl",
        Some(provenance_core::NodeType::Topic),
        Some("topics"),
        Some(70),
        Resource,
        &["list-topics", "page-topics-v2", "get-topic-v2"],
    ),
    family(
        Questions,
        Export,
        "questions",
        "questions/question.jsonl",
        Some(provenance_core::NodeType::Question),
        Some("questions"),
        Some(80),
        Resource,
        &["list-questions", "page-questions-v2", "get-question-v2"],
    ),
    family(
        Resolutions,
        Export,
        "resolutions",
        "resolutions/res.jsonl",
        Some(provenance_core::NodeType::Resolution),
        Some("resolutions"),
        Some(30),
        Resource,
        &[
            "list-resolutions",
            "page-resolutions-v2",
            "get-resolution-v2",
        ],
    ),
    family(
        Rules,
        Export,
        "rules",
        "rules/rule.jsonl",
        Some(provenance_core::NodeType::Rule),
        Some("rules"),
        Some(40),
        Resource,
        &["list-rules", "page-rules-v2", "get-rule-v2"],
    ),
    family(
        Threads,
        Canonical,
        "threads",
        "threads/threads.jsonl",
        None,
        None,
        Some(140),
        Record,
        &[
            "list-discussion-containers",
            "page-discussion-containers-v2",
            "get-discussion-container-v2",
        ],
    ),
    family(
        Messages,
        Canonical,
        "messages",
        "threads/2026-07.jsonl",
        None,
        None,
        Some(150),
        Record,
        &["list-messages-v2", "page-messages-v2", "get-message-v2"],
    ),
    family(
        Contributions,
        Canonical,
        "contributions",
        "ideation/contributions.jsonl",
        None,
        None,
        Some(90),
        Resource,
        &[
            "list-contributions",
            "page-contributions-v2",
            "get-contribution-v2",
        ],
    ),
    family(
        SynthesisPackets,
        Canonical,
        "synthesis_packets",
        "ideation/synthesis_packets.jsonl",
        None,
        None,
        Some(100),
        Resource,
        &[
            "list-synthesis-packets",
            "page-synthesis-packets-v2",
            "get-synthesis-packet-v2",
        ],
    ),
    family(
        ProposalCards,
        Canonical,
        "proposal_cards",
        "ideation/proposal_cards.jsonl",
        None,
        None,
        Some(110),
        Resource,
        &["list-proposals-v2", "page-proposals-v2", "get-proposal-v2"],
    ),
    family(
        AssertionRecords,
        Canonical,
        "assertion_records",
        "ideation/assertions.jsonl",
        None,
        None,
        Some(160),
        Resource,
        &[
            "list-assertions-v2",
            "page-assertions-v2",
            "get-assertion-v2",
        ],
    ),
    family(
        Dispositions,
        Canonical,
        "dispositions",
        "ideation/dispositions.jsonl",
        None,
        None,
        Some(170),
        Resource,
        &[
            "list-dispositions-v2",
            "page-dispositions-v2",
            "get-disposition-v2",
        ],
    ),
    family(
        ImplementationBindings,
        Binding,
        "implementation_bindings",
        "implementations/binding.jsonl",
        None,
        Some("implementation_bindings"),
        None,
        Unchecked,
        &[],
    ),
    family(
        VerificationBindings,
        Binding,
        "verification_bindings",
        "verifications/binding.jsonl",
        None,
        Some("verification_bindings"),
        Some(130),
        Resource,
        &[
            "list-verification-bindings",
            "page-verification-bindings-v2",
            "get-verification-binding-v2",
        ],
    ),
    family(
        RequirementReviews,
        Internal,
        "requirement_reviews",
        "requirements/review.jsonl",
        None,
        None,
        None,
        NotImported,
        &[],
    ),
    family(
        ReviewJournal,
        Internal,
        "review_journal",
        "review/journal",
        None,
        None,
        None,
        NotImported,
        &[],
    ),
];

const fn family(
    family: ProjectionFamily,
    group: FamilyGroup,
    table_name: &'static str,
    shard_suffix: &'static str,
    node_type: Option<provenance_core::NodeType>,
    graph_field: Option<&'static str>,
    route_order: Option<u16>,
    budget: BudgetKind,
    catalog_operations: &'static [&'static str],
) -> FamilyMeta {
    FamilyMeta {
        family,
        group,
        table_name,
        shard_suffix,
        node_type,
        graph_field,
        route_order,
        budget,
        catalog_operations,
    }
}

impl ProjectionFamily {
    pub(crate) const fn meta(self) -> &'static FamilyMeta {
        &FAMILIES[self as usize]
    }
}

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
mod tests;
