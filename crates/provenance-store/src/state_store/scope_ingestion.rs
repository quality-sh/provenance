use super::read_budget::{ensure_slice_within_read_budget, ReadBudget};
use super::{
    ensure_new_ids_assignable, overlay_records, read_jsonl_unlocked, read_message_shards_unlocked,
    serde_name, IdeationLandingBatch, StateStore,
};
use crate::cache::{BudgetKind, ProjectionFamily, RecordFamily};
use crate::jsonl::write_jsonl_atomic_under_publication;
use crate::shards;
use provenance_core::{
    AssertionRecord, Contribution, DispositionRecord, NodeType, ProposalCard, ScopeId,
    SynthesisPacket, Thread, ThreadStatus,
};

macro_rules! collect_node_ids {
    ($records:ident, $kinds:ident, $shards:ident, $field:ident, [$node:ident]) => {
        $kinds.push(NodeType::$node);
        $records.extend(
            $shards
                .$field
                .iter()
                .map(|record| (NodeType::$node, &record.id)),
        );
    };
    ($records:ident, $kinds:ident, $shards:ident, $field:ident, []) => {};
}

fn ensure_family_budget<T: ReadBudget + RecordFamily>(records: &[T]) -> anyhow::Result<()> {
    if T::META.budget != BudgetKind::Unchecked {
        ensure_slice_within_read_budget(records)?;
    }
    Ok(())
}

macro_rules! ensure_assignable {
    (
        $store:ident, $scope:ident, $shards:ident, $field:ident,
        $path:ident, $strategy:ident, node_budget
    ) => {};
    ($store:ident, $scope:ident, $shards:ident, $field:ident, $path:ident, direct, assignable) => {
        ensure_new_ids_assignable(
            &read_jsonl_unlocked(&crate::shards::$path(&$store.layout, $scope))?,
            $shards.$field,
            |record| record.id.as_str(),
        )?;
    };
    (
        $store:ident, $scope:ident, $shards:ident, $field:ident,
        $path:ident, $strategy:ident, assignable_budget
    ) => {
        ensure_assignable!($store, $scope, $shards, $field, $path, $strategy, assignable);
    };
    (
        $store:ident, $scope:ident, $shards:ident, $field:ident,
        $path:ident, direct, threads_assignable_budget
    ) => {
        ensure_assignable!($store, $scope, $shards, $field, $path, direct, assignable);
    };
    (
        $store:ident, $scope:ident, $shards:ident, $field:ident,
        $path:ident, messages, messages_assignable_budget
    ) => {
        ensure_new_ids_assignable(
            &read_message_shards_unlocked(&$store.layout, $scope)?,
            $shards.$field,
            |record| record.id.as_str(),
        )?;
    };
    (
        $store:ident, $scope:ident, $shards:ident, $field:ident,
        $path:ident, contributions, assignable
    ) => {
        ensure_new_ids_assignable(
            &read_ideation_records_unlocked($store, $scope)?.contributions,
            $shards.$field,
            |record| record.id.as_str(),
        )?;
    };
    (
        $store:ident, $scope:ident, $shards:ident, $field:ident,
        $path:ident, synthesis_packets, assignable
    ) => {
        ensure_new_ids_assignable(
            &read_ideation_records_unlocked($store, $scope)?.synthesis_packets,
            $shards.$field,
            |record| record.id.as_str(),
        )?;
    };
    (
        $store:ident, $scope:ident, $shards:ident, $field:ident,
        $path:ident, proposal_cards, assignable
    ) => {
        ensure_new_ids_assignable(
            &read_ideation_records_unlocked($store, $scope)?.proposals,
            $shards.$field,
            |record| record.id.as_str(),
        )?;
    };
    (
        $store:ident, $scope:ident, $shards:ident, $field:ident,
        $path:ident, assertion_records, assignable
    ) => {
        ensure_new_ids_assignable(
            &read_ideation_records_unlocked($store, $scope)?.assertions,
            $shards.$field,
            |record| record.id.as_str(),
        )?;
    };
    (
        $store:ident, $scope:ident, $shards:ident, $field:ident,
        $path:ident, dispositions, assignable
    ) => {
        ensure_new_ids_assignable(
            &read_ideation_records_unlocked($store, $scope)?.dispositions,
            $shards.$field,
            |record| record.id.as_str(),
        )?;
    };
}

macro_rules! write_binding {
    (
        verification, $store:ident, $scope:ident, $shards:ident,
        $variant:ident, $field:ident, verification($kind:literal)
    ) => {
        write_family!($store, $scope, $shards, $variant, $field);
    };
    (
        implementation, $store:ident, $scope:ident, $shards:ident,
        $variant:ident, $field:ident, implementation($kind:literal)
    ) => {
        write_family!($store, $scope, $shards, $variant, $field);
    };
    (
        verification, $store:ident, $scope:ident, $shards:ident,
        $variant:ident, $field:ident, implementation($kind:literal)
    ) => {};
    (
        implementation, $store:ident, $scope:ident, $shards:ident,
        $variant:ident, $field:ident, verification($kind:literal)
    ) => {};
}

macro_rules! write_family {
    ($store:ident, $scope:ident, $shards:ident, $variant:ident, $field:ident) => {
        write_jsonl_atomic_under_publication(
            &ProjectionFamily::$variant.shard_path(&$store.layout, $scope),
            $shards.$field,
        )?;
        crate::test_probes::at(concat!("scope_import_", stringify!($field), "_written"))?;
    };
}

macro_rules! define_imported_scope_shards {
    (
        export { $(
            $export_variant:ident {
                record: $export_type:ty,
                field: $export_field:ident,
                path: $export_path:ident,
                node: [$($export_node:tt)*],
                reader: {
                    open: $export_reader:ident,
                    closed: [$($export_closed:tt)*],
                    strategy: $export_strategy:ident
                },
                id: $export_id:ident,
                loader: [$($export_loader:tt)*],
                graph: [$($export_graph:tt)*],
                import: [$export_import:ident],
                catalog: [$($export_catalog:tt)*],
                route: [$($export_route:tt)*]
            };
        )* }
        canonical { $(
            $canonical_variant:ident {
                record: $canonical_type:ty,
                field: $canonical_field:ident,
                path: $canonical_path:ident,
                node: [$($canonical_node:tt)*],
                reader: {
                    open: $canonical_reader:ident,
                    closed: [$($canonical_closed:tt)*],
                    strategy: $canonical_strategy:ident
                },
                id: $canonical_id:ident,
                loader: [$($canonical_loader:tt)*],
                graph: [$($canonical_graph:tt)*],
                import: [$canonical_import:ident],
                catalog: [$($canonical_catalog:tt)*],
                route: [$($canonical_route:tt)*]
            };
        )* }
        bindings { $(
            $binding_variant:ident {
                record: $binding_type:ty,
                field: $binding_field:ident,
                path: $binding_path:ident,
                node: [$($binding_node:tt)*],
                reader: {
                    open: $binding_reader:ident,
                    closed: [$($binding_closed:tt)*],
                    strategy: $binding_strategy:ident
                },
                id: $binding_id:ident,
                loader: [$($binding_loader:tt)*],
                graph: [$($binding_graph:tt)+],
                import: [$binding_import:ident],
                catalog: [$($binding_catalog:tt)*],
                route: [$($binding_route:tt)*]
            };
        )* }
        internal { $($internal:tt)* }
    ) => {
        /// The complete canonical shard contents for one scope import.
        #[derive(Default)]
        pub struct ScopeShards<'a> {
            $(pub $export_field: &'a [$export_type],)*
            $(pub $canonical_field: &'a [$canonical_type],)*
            $(pub $binding_field: &'a [$binding_type],)*
        }

        impl StateStore {
            /// Writes the canonical record shards for one scope.
            pub fn import_scope(
                &self,
                scope: &ScopeId,
                shards: &ScopeShards<'_>,
            ) -> anyhow::Result<()> {
                validate_threads(shards.threads)?;
                $(ensure_family_budget(shards.$export_field)?;)*
                $(ensure_family_budget(shards.$canonical_field)?;)*
                $(ensure_family_budget(shards.$binding_field)?;)*

                let mut records = Vec::new();
                let mut kinds = Vec::new();
                $(collect_node_ids!(records, kinds, shards, $export_field, [$($export_node)*]);)*
                self.ensure_import_ids_unique(scope, records, &kinds)?;
                $(ensure_assignable!(
                    self, scope, shards, $canonical_field, $canonical_path,
                    $canonical_strategy, $canonical_import
                );)*
                $(ensure_assignable!(
                    self, scope, shards, $binding_field, $binding_path,
                    $binding_strategy, $binding_import
                );)*

                let scope_dir = self.layout.scopes_dir().join(scope.as_str());
                if scope_dir.exists() {
                    std::fs::remove_dir_all(&scope_dir)?;
                }
                $(write_family!(self, scope, shards, $export_variant, $export_field);)*
                $(write_binding!(
                    verification, self, scope, shards, $binding_variant,
                    $binding_field, $($binding_graph)+
                );)*
                $(write_binding!(
                    implementation, self, scope, shards, $binding_variant,
                    $binding_field, $($binding_graph)+
                );)*
                $(write_family!(self, scope, shards, $canonical_variant, $canonical_field);)*
                Ok(())
            }
        }
    };
}

crate::cache::family_table::record_family_rows!(define_imported_scope_shards);

struct StoredIdeationRecords {
    contributions: Vec<Contribution>,
    synthesis_packets: Vec<SynthesisPacket>,
    proposals: Vec<ProposalCard>,
    assertions: Vec<AssertionRecord>,
    dispositions: Vec<DispositionRecord>,
}

fn read_ideation_records_unlocked(
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<StoredIdeationRecords> {
    let layout = &store.layout;
    let mut records = StoredIdeationRecords {
        contributions: read_jsonl_unlocked(&shards::contributions_path(layout, scope))?,
        synthesis_packets: read_jsonl_unlocked(&shards::synthesis_packets_path(layout, scope))?,
        proposals: read_jsonl_unlocked(&shards::proposal_cards_path(layout, scope))?,
        assertions: read_jsonl_unlocked(&shards::assertion_records_path(layout, scope))?,
        dispositions: read_jsonl_unlocked(&shards::dispositions_path(layout, scope))?,
    };
    records
        .dispositions
        .extend(super::readers::read_legacy_dispositions_unlocked(
            &shards::legacy_promotion_decisions_path(layout, scope),
        )?);
    let landings: Vec<IdeationLandingBatch> = super::readers::read_ideation_landings_unlocked(
        &shards::ideation_landings_path(layout, scope),
    )?;
    for batch in landings {
        overlay_records(&mut records.contributions, batch.contributions, |r| {
            r.id.as_str()
        });
        overlay_records(
            &mut records.synthesis_packets,
            batch.synthesis_packets,
            |r| r.id.as_str(),
        );
        overlay_records(&mut records.proposals, batch.proposals, |r| r.id.as_str());
        overlay_records(&mut records.assertions, batch.assertions, |r| r.id.as_str());
        overlay_records(&mut records.dispositions, batch.dispositions, |r| {
            r.id.as_str()
        });
    }
    Ok(records)
}

fn validate_threads(threads: &[Thread]) -> anyhow::Result<()> {
    for (index, thread) in threads.iter().enumerate() {
        if threads[..index]
            .iter()
            .any(|earlier| earlier.id == thread.id)
        {
            anyhow::bail!("duplicate thread id {}", thread.id.as_str());
        }
        if thread.status != ThreadStatus::Active {
            continue;
        }
        if let Some(earlier) = threads[..index].iter().find(|earlier| {
            earlier.status == ThreadStatus::Active && earlier.parent == thread.parent
        }) {
            let node_type = serde_name(&thread.parent.node_type)?;
            anyhow::bail!(
                "multiple active threads for {} {}: {} and {}",
                node_type,
                thread.parent.node_id.as_str(),
                earlier.id.as_str(),
                thread.id.as_str()
            );
        }
    }
    Ok(())
}
