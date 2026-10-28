use super::{ensure_graph_schema_version, incomplete, GraphReferenceError};
use crate::{layout::ProvenanceLayout, state_store::StateStore};
use camino::Utf8Path;
use provenance_core::{Scope, ScopeId, SUPPORTED_SCHEMA_VERSION};
use provenance_macros::rule;
use serde::{ser::SerializeStruct, Deserialize, Serialize};

macro_rules! graph_kind {
    (export($kind:literal, $test_kind:ident, $fixture:ident, $id:literal)) => {
        $kind
    };
    (verification($kind:literal, $test_kind:ident, $fixture:ident, $id:literal)) => {
        $kind
    };
    (implementation($kind:literal, $test_kind:ident, $fixture:ident, $id:literal)) => {
        $kind
    };
}

macro_rules! serialize_binding {
    (
        $graph:ident, $state:ident, verification,
        verification($kind:literal, $test_kind:ident, $fixture:ident, $id:literal), $field:ident
    ) => {
        if !$graph.$field.is_empty() {
            $state.serialize_field(stringify!($field), &$graph.$field)?;
        }
    };
    (
        $graph:ident, $state:ident, implementation,
        implementation($kind:literal, $test_kind:ident, $fixture:ident, $id:literal), $field:ident
    ) => {
        if !$graph.$field.is_empty() {
            $state.serialize_field(stringify!($field), &$graph.$field)?;
        }
    };
    (
        $graph:ident, $state:ident, $phase:ident,
        $other:ident($kind:literal, $test_kind:ident, $fixture:ident, $id:literal), $field:ident
    ) => {};
}

macro_rules! read_binding {
    (
        $graph:ident, $store:ident, $scope:ident, verification,
        verification($kind:literal, $test_kind:ident, $fixture:ident, $id:literal),
        $field:ident, $reader:ident
    ) => {
        $graph.$field = $store.$reader(&$scope).map_err(incomplete)?;
    };
    (
        $graph:ident, $store:ident, $scope:ident, implementation,
        implementation($kind:literal, $test_kind:ident, $fixture:ident, $id:literal),
        $field:ident, $reader:ident
    ) => {
        $graph.$field = $store.$reader(&$scope).map_err(incomplete)?;
    };
    (
        $graph:ident, $store:ident, $scope:ident, $phase:ident,
        $other:ident($kind:literal, $test_kind:ident, $fixture:ident, $id:literal),
        $field:ident, $reader:ident
    ) => {};
}

macro_rules! validate_binding {
    (
        $graph:ident, verification,
        verification($kind:literal, $test_kind:ident, $fixture:ident, $id:literal),
        $field:ident, $check:ident
    ) => {
        $check!(&$graph.$field, $kind);
    };
    (
        $graph:ident, implementation,
        implementation($kind:literal, $test_kind:ident, $fixture:ident, $id:literal),
        $field:ident, $check:ident
    ) => {
        $check!(&$graph.$field, $kind);
    };
    (
        $graph:ident, $phase:ident,
        $other:ident($kind:literal, $test_kind:ident, $fixture:ident, $id:literal),
        $field:ident, $check:ident
    ) => {};
}

macro_rules! count_fields {
    () => { 0usize };
    ($field:ident $($rest:ident)*) => { 1usize + count_fields!($($rest)*) };
}

macro_rules! define_graph_projection {
    (
        export { $(
            $export_variant:ident {
                record: $export_type:ty,
                field: $export_field:ident,
                path: $export_path:ident,
                meta: $export_meta:tt,
                node: [$($export_node:tt)*],
                reader: {
                    open: $export_reader:ident,
                    closed: [$export_closed:ident],
                    strategy: $export_strategy:ident
                },
                id: $export_id:ident,
                loader: [$($export_loader:tt)*],
                graph: [$($export_graph:tt)+],
                import: [$($export_import:tt)*],
                catalog: [$($export_catalog:tt)*],
                route: [$($export_route:tt)*]
                $(, review: $export_review:ident)?
            };
        )* }
        canonical { $($canonical:tt)* }
        bindings { $(
            $binding_variant:ident {
                record: $binding_type:ty,
                field: $binding_field:ident,
                path: $binding_path:ident,
                meta: $binding_meta:tt,
                node: [$($binding_node:tt)*],
                reader: {
                    open: $binding_reader:ident,
                    closed: [$binding_closed:ident],
                    strategy: $binding_strategy:ident
                },
                id: $binding_id:ident,
                loader: [$($binding_loader:tt)*],
                graph: [$($binding_graph:tt)+],
                import: [$($binding_import:tt)*],
                catalog: [$($binding_catalog:tt)*],
                route: [$($binding_route:tt)*]
                $(, review: $binding_review:ident)?
            };
        )* }
        internal { $($internal:tt)* }
    ) => {
        /// The pinned graph contains these canonical record families.
        #[rule("rule_pinned_graph_families")]
        #[derive(Debug, Clone, PartialEq, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct GraphExport {
            pub schema_version: u32,
            pub scope: Scope,
            $(pub $export_field: Vec<$export_type>,)*
            $(#[serde(default)] pub $binding_field: Vec<$binding_type>,)*
        }

        impl Serialize for GraphExport {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let field_count = 2
                    + count_fields!($($export_field)*)
                    + count_fields!($($binding_field)*);
                let mut state = serializer.serialize_struct("GraphExport", field_count)?;
                state.serialize_field("schema_version", &self.schema_version)?;
                state.serialize_field("scope", &self.scope)?;
                $(state.serialize_field(stringify!($export_field), &self.$export_field)?;)*
                $(serialize_binding!(
                    self, state, verification, $($binding_graph)+, $binding_field
                );)*
                $(serialize_binding!(
                    self, state, implementation, $($binding_graph)+, $binding_field
                );)*
                state.end()
            }
        }

        pub(super) fn load_projection(
            repository: &Utf8Path,
            scope: &str,
        ) -> Result<GraphExport, GraphReferenceError> {
            let scope_id = ScopeId::new(scope.to_string()).map_err(incomplete)?;
            let store = StateStore::new(ProvenanceLayout::new(repository.to_path_buf()));
            let (manifest_version, selected_scope) =
                store.closed_manifest_scope(&scope_id).map_err(|error| {
                    let detail = error.to_string();
                    if repository.join(".provenance/state/manifest.json").exists() {
                        incomplete(detail)
                    } else {
                        GraphReferenceError::Missing {
                            detail: "canonical manifest is absent".into(),
                        }
                    }
                })?;
            ensure_graph_schema_version("manifest", manifest_version)?;
            let selected_scope = selected_scope.ok_or_else(|| GraphReferenceError::Missing {
                detail: format!("scope '{scope}' is absent from the pinned manifest"),
            })?;
            let mut graph = GraphExport {
                schema_version: SUPPORTED_SCHEMA_VERSION.0,
                scope: selected_scope,
                $($export_field: store.$export_closed(&scope_id).map_err(incomplete)?,)*
                $($binding_field: Vec::new(),)*
            };
            $(read_binding!(
                graph, store, scope_id, verification, $($binding_graph)+,
                $binding_field, $binding_closed
            );)*
            $(read_binding!(
                graph, store, scope_id, implementation, $($binding_graph)+,
                $binding_field, $binding_closed
            );)*
            graph.validate_schema_versions()?;
            graph.validate_rule_archives()?;
            validate_scope_ownership(&graph, &scope_id)?;
            strip_collaboration_fields(&mut graph);
            sort_records(&mut graph);
            Ok(graph)
        }

        impl GraphExport {
            pub(super) fn validate_schema_versions(&self) -> Result<(), GraphReferenceError> {
                macro_rules! require_supported {
                    ($records:expr, $kind:expr) => {
                        for record in $records {
                            ensure_graph_schema_version(
                                &format!("{} '{}'", $kind, record.id.as_str()),
                                record.schema_version,
                            )?;
                        }
                    };
                }
                $(require_supported!(&self.$export_field, graph_kind!($($export_graph)+));)*
                $(validate_binding!(
                    self, verification, $($binding_graph)+, $binding_field, require_supported
                );)*
                $(validate_binding!(
                    self, implementation, $($binding_graph)+, $binding_field, require_supported
                );)*
                Ok(())
            }
        }

        fn sort_records(graph: &mut GraphExport) {
            $(graph.$export_field.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));)*
            $(graph.$binding_field.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));)*
        }

        /// Checks that each pinned record belongs to the selected scope.
        #[rule("rule_pinned_scope_ownership")]
        pub(super) fn validate_scope_ownership(
            graph: &GraphExport,
            scope: &ScopeId,
        ) -> Result<(), GraphReferenceError> {
            macro_rules! require_scope {
                ($records:expr, $kind:expr) => {
                    for record in $records {
                        if &record.scope_id != scope {
                            return Err(GraphReferenceError::Incomplete {
                                detail: format!(
                                    "{} '{}' belongs to scope '{}', not '{}'",
                                    $kind,
                                    record.id.as_str(),
                                    record.scope_id.as_str(),
                                    scope.as_str()
                                ),
                            });
                        }
                    }
                };
            }
            $(require_scope!(&graph.$export_field, graph_kind!($($export_graph)+));)*
            $(validate_binding!(
                graph, verification, $($binding_graph)+, $binding_field, require_scope
            );)*
            $(validate_binding!(
                graph, implementation, $($binding_graph)+, $binding_field, require_scope
            );)*
            Ok(())
        }
    };
}

crate::cache::family_table::record_family_rows!(define_graph_projection);

impl GraphExport {
    pub(super) fn validate_rule_archives(&self) -> Result<(), GraphReferenceError> {
        for rule in &self.rules {
            rule.validate_archive().map_err(|error| {
                incomplete(format!(
                    "rule '{}' is archive-inconsistent: {error}",
                    rule.id.as_str()
                ))
            })?;
        }
        Ok(())
    }

    /// Refuses a graph that arrives still carrying collaboration state.
    ///
    /// The import half of `visit_collaboration_fields`: it asks the same
    /// walk which fields exist and refuses the graph if any of them is
    /// set, naming the first one so the holder can find it.
    pub(super) fn validate_no_collaboration_fields(&mut self) -> Result<(), GraphReferenceError> {
        let mut populated = None;
        visit_collaboration_fields(self, &mut |name, field| {
            if populated.is_none() && field.is_populated() {
                populated = Some(name);
            }
        });
        populated.map_or(Ok(()), |name| {
            Err(GraphReferenceError::Incomplete {
                detail: format!("exact export must not contain collaboration field '{name}'"),
            })
        })
    }
}

/// A record field holding collaboration state, seen without its type.
///
/// The walk below hands each field out as one of these so a visitor can
/// ask whether it is set, or clear it, without knowing whether it holds a
/// record id, an actor name, or a timestamp.
trait CollaborationField {
    fn is_populated(&self) -> bool;
    fn clear(&mut self);
}

impl<T> CollaborationField for Option<T> {
    fn is_populated(&self) -> bool {
        self.is_some()
    }

    fn clear(&mut self) {
        *self = None;
    }
}

/// Decides which fields a graph must shed before it leaves the repository.
///
/// A graph shared outside the repository carries no trace of who was
/// talking or who claimed what. Thread and message origins, and the claims
/// on topics and questions, are collaboration state: they name people and
/// conversations, and the graph does not contain them.
///
/// This walk is the only place that list of fields is written down. Export
/// clears every field it visits and import refuses a graph in which any field
/// it visits is set, so the two halves cannot drift: a field added here is
/// both stripped and rejected at once, and a field missing here is neither.
///
/// Visiting is field-major, every record of a kind for one field before the
/// next field, so which field a refusal names does not depend on which
/// record happens to carry it.
#[rule("rule_export_strips_collaboration")]
fn visit_collaboration_fields(
    graph: &mut GraphExport,
    visit: &mut dyn FnMut(&'static str, &mut dyn CollaborationField),
) {
    macro_rules! visit_field {
        ($records:expr, $field:ident) => {
            for record in $records {
                visit(stringify!($field), &mut record.$field);
            }
        };
    }
    visit_field!(&mut graph.sources, origin_thread);
    visit_field!(&mut graph.sources, origin_message);
    visit_field!(&mut graph.requirements, origin_thread);
    visit_field!(&mut graph.requirements, origin_message);
    visit_field!(&mut graph.topics, claimed_by);
    visit_field!(&mut graph.topics, claimed_at);
    visit_field!(&mut graph.questions, claimed_by);
    visit_field!(&mut graph.questions, claimed_at);
    visit_field!(&mut graph.resolutions, origin_thread);
    visit_field!(&mut graph.resolutions, origin_message);
    visit_field!(&mut graph.rules, origin_thread);
    visit_field!(&mut graph.rules, origin_message);
}

/// Clears every collaboration field on the way out.
///
/// The export half of `visit_collaboration_fields`.
fn strip_collaboration_fields(graph: &mut GraphExport) {
    visit_collaboration_fields(graph, &mut |_, field| field.clear());
}

#[cfg(test)]
mod tests;
