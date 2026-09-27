use crate::layout::ProvenanceLayout;
use crate::state_store::{GuardedStore, StateStore};
use camino::Utf8PathBuf;
use provenance_core::ScopeId;

pub(crate) trait RecordFamily: serde::Serialize {
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
            const META: &'static FamilyMeta = &FAMILIES[ProjectionFamily::$variant as usize];

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
            pub const ALL: [Self; count_families!($($($variant)*)*)] = [
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
        }
    };
}

macro_rules! count_families {
    () => { 0usize };
    ($family:ident $($rest:ident)*) => { 1usize + count_families!($($rest)*) };
}

macro_rules! family_record_id {
    ($record:expr, field) => {
        $record.id.as_str()
    };
    ($record:expr, method) => {
        $record.id().as_str()
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
    pub group: FamilyGroup,
    pub table_name: &'static str,
    pub shard_suffix: &'static str,
    pub node_type: Option<provenance_core::NodeType>,
    pub graph_field: Option<&'static str>,
    pub route_order: Option<u16>,
    pub budget: BudgetKind,
    #[allow(dead_code)]
    pub catalog_operations: &'static [&'static str],
}

use BudgetKind::{NotImported, Record, Resource, Unchecked};
use FamilyGroup::{Binding, Canonical, Export, Internal};
pub(crate) const FAMILIES: &[FamilyMeta] = &[
    family(
        Export,
        ("sources", "sources/source.jsonl"),
        Some(provenance_core::NodeType::Source),
        Some("sources"),
        Some(10),
        Resource,
        &["list-sources", "page-sources-v2", "get-source-v2"],
    ),
    family(
        Export,
        ("domains", "domains/domain.jsonl"),
        Some(provenance_core::NodeType::Domain),
        Some("domains"),
        Some(50),
        Resource,
        &["list-domains", "page-domains-v2", "get-domain-v2"],
    ),
    family(
        Export,
        ("requirements", "requirements/req.jsonl"),
        Some(provenance_core::NodeType::Requirement),
        Some("requirements"),
        Some(20),
        Resource,
        &["list-requirements", "page-requirements-v2"],
    ),
    family(
        Export,
        ("boundaries", "boundaries/boundary.jsonl"),
        Some(provenance_core::NodeType::Boundary),
        Some("boundaries"),
        Some(60),
        Resource,
        &["list-boundaries", "page-boundaries-v2", "get-boundary-v2"],
    ),
    family(
        Export,
        ("topics", "topics/topic.jsonl"),
        Some(provenance_core::NodeType::Topic),
        Some("topics"),
        Some(70),
        Resource,
        &["list-topics", "page-topics-v2", "get-topic-v2"],
    ),
    family(
        Export,
        ("questions", "questions/question.jsonl"),
        Some(provenance_core::NodeType::Question),
        Some("questions"),
        Some(80),
        Resource,
        &["list-questions", "page-questions-v2", "get-question-v2"],
    ),
    family(
        Export,
        ("resolutions", "resolutions/res.jsonl"),
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
        Export,
        ("rules", "rules/rule.jsonl"),
        Some(provenance_core::NodeType::Rule),
        Some("rules"),
        Some(40),
        Resource,
        &["list-rules", "page-rules-v2", "get-rule-v2"],
    ),
    family(
        Canonical,
        ("threads", "threads/threads.jsonl"),
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
        Canonical,
        ("messages", "threads/2026-07.jsonl"),
        None,
        None,
        Some(150),
        Record,
        &["list-messages-v2", "page-messages-v2", "get-message-v2"],
    ),
    family(
        Canonical,
        ("contributions", "ideation/contributions.jsonl"),
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
        Canonical,
        ("synthesis_packets", "ideation/synthesis_packets.jsonl"),
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
        Canonical,
        ("proposal_cards", "ideation/proposal_cards.jsonl"),
        None,
        None,
        Some(110),
        Resource,
        &["list-proposals-v2", "page-proposals-v2", "get-proposal-v2"],
    ),
    family(
        Canonical,
        ("assertion_records", "ideation/assertions.jsonl"),
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
        Canonical,
        ("dispositions", "ideation/dispositions.jsonl"),
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
        Binding,
        ("implementation_bindings", "implementations/binding.jsonl"),
        None,
        Some("implementation_bindings"),
        None,
        Unchecked,
        &[],
    ),
    family(
        Binding,
        ("verification_bindings", "verifications/binding.jsonl"),
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
        Internal,
        ("requirement_reviews", "requirements/review.jsonl"),
        None,
        None,
        None,
        NotImported,
        &[],
    ),
    family(
        Internal,
        ("review_journal", "review/journal"),
        None,
        None,
        None,
        NotImported,
        &[],
    ),
];

const fn family(
    group: FamilyGroup,
    storage: (&'static str, &'static str),
    node_type: Option<provenance_core::NodeType>,
    graph_field: Option<&'static str>,
    route_order: Option<u16>,
    budget: BudgetKind,
    catalog_operations: &'static [&'static str],
) -> FamilyMeta {
    FamilyMeta {
        group,
        table_name: storage.0,
        shard_suffix: storage.1,
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

fn sorted_bytes<T: RecordFamily>(mut records: Vec<T>) -> anyhow::Result<(Vec<u8>, u64)> {
    records.sort_by(|left, right| left.record_id().cmp(right.record_id()));
    let count = records.len() as u64;
    Ok((crate::canonical_digest::canonical_bytes(&records)?, count))
}

#[cfg(test)]
mod tests;
