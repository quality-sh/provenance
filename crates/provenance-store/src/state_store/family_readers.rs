//! The per-family list readers that the record family tables generate.

use super::ideation_batches::overlay_records;
use super::readers::{read_jsonl, read_message_shards};
use super::shared_parses::{read_jsonl_shared, shared};
use super::StateStore;
use crate::shards;
use provenance_core::{Rule, ScopeId};

macro_rules! family_reader {
    (Rules, $record:ty, $path:ident, $list:ident) => {
        pub fn $list(&self, scope: &ScopeId) -> anyhow::Result<Vec<$record>> {
            let path = shards::$path(&self.layout, scope);
            validate_rule_archives(shared(self, &path, || read_jsonl(self, &path))?)
        }
    };
    ($variant:ident, $record:ty, $path:ident, $list:ident) => {
        pub fn $list(&self, scope: &ScopeId) -> anyhow::Result<Vec<$record>> {
            let path = shards::$path(&self.layout, scope);
            shared(self, &path, || read_jsonl(self, &path))
        }
    };
}

macro_rules! define_export_readers {
    (
        export { $(
            $variant:ident {
                record: $record:ty,
                field: $field:ident,
                path: $path:ident,
                node: [$($node:tt)*],
                reader: $list:ident,
                $($export_rest:tt)*
            };
        )* }
        canonical { $($canonical:tt)* }
        bindings { $(
            $binding_variant:ident {
                record: $binding_record:ty,
                field: $binding_field:ident,
                path: $binding_path:ident,
                node: [$($binding_node:tt)*],
                reader: $binding_list:ident,
                $($binding_rest:tt)*
            };
        )* }
        internal { $($internal:tt)* }
    ) => {
        $(family_reader!($variant, $record, $path, $list);)*
        $(family_reader!(
            $binding_variant,
            $binding_record,
            $binding_path,
            $binding_list
        );)*
    };
}

macro_rules! canonical_reader {
    ($record:ty, $path:ident, $list:ident, direct) => {
        pub fn $list(&self, scope: &ScopeId) -> anyhow::Result<Vec<$record>> {
            read_jsonl(self, &shards::$path(&self.layout, scope))
        }
    };
    ($record:ty, $path:ident, $list:ident, messages) => {
        pub fn $list(&self, scope: &ScopeId) -> anyhow::Result<Vec<$record>> {
            read_message_shards(self, &self.layout, scope)
        }
    };
    ($record:ty, $path:ident, $list:ident, contributions) => {
        pub fn $list(&self, scope: &ScopeId) -> anyhow::Result<Vec<$record>> {
            self.list_contributions_after_direct_read(scope, || Ok(()))
        }
    };
    ($record:ty, $path:ident, $list:ident, synthesis_packets) => {
        pub fn $list(&self, scope: &ScopeId) -> anyhow::Result<Vec<$record>> {
            self.with_repository_read(|held| {
                let mut records = read_jsonl_shared(held, &shards::$path(&held.layout, scope))?;
                for batch in held.list_ideation_landings(scope)? {
                    overlay_records(&mut records, batch.synthesis_packets, |record| {
                        record.id.as_str()
                    });
                }
                Ok(records)
            })
        }
    };
    ($record:ty, $path:ident, $list:ident, proposal_cards) => {
        pub fn $list(&self, scope: &ScopeId) -> anyhow::Result<Vec<$record>> {
            self.project_proposal_cards(scope, None, || Ok(()))
        }
    };
    ($record:ty, $path:ident, $list:ident, assertion_records) => {
        pub fn $list(&self, scope: &ScopeId) -> anyhow::Result<Vec<$record>> {
            self.with_repository_read(|held| {
                let mut records = read_jsonl_shared(held, &shards::$path(&held.layout, scope))?;
                for batch in held.list_ideation_landings(scope)? {
                    overlay_records(&mut records, batch.assertions, |record| record.id.as_str());
                }
                Ok(records)
            })
        }
    };
    ($record:ty, $path:ident, $list:ident, dispositions) => {
        pub fn $list(&self, scope: &ScopeId) -> anyhow::Result<Vec<$record>> {
            self.with_repository_read(|held| {
                let mut records = read_jsonl_shared(held, &shards::$path(&held.layout, scope))?;
                for batch in held.list_ideation_landings(scope)? {
                    overlay_records(&mut records, batch.dispositions, |record| {
                        record.id.as_str()
                    });
                }
                Ok(records)
            })
        }
    };
}

macro_rules! define_canonical_readers {
    (
        export { $($export:tt)* }
        canonical { $(
            $variant:ident {
                record: $record:ty,
                field: $field:ident,
                path: $path:ident,
                meta: $meta:tt,
                node: [$($node:tt)*],
                reader: {
                    open: $reader:ident,
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
        )* }
        bindings { $($bindings:tt)* }
        internal { $($internal:tt)* }
    ) => {
        $(canonical_reader!($record, $path, $reader, $strategy);)*
    };
}

impl StateStore {
    crate::families::record_families!(define_export_readers);
    crate::families::family_table::record_family_rows!(define_canonical_readers);
}

fn validate_rule_archives(rules: Vec<Rule>) -> anyhow::Result<Vec<Rule>> {
    for rule in &rules {
        rule.validate_archive().map_err(|error| {
            anyhow::anyhow!("rule {} is archive-inconsistent: {error}", rule.id.as_str())
        })?;
    }
    Ok(rules)
}
