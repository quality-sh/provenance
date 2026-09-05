use super::{ensure_graph_schema_version, GraphReferenceError};
use crate::{layout::ProvenanceLayout, state_store::StateStore};
use camino::Utf8Path;
use provenance_core::{
    Boundary, Domain, ImplementationBinding, Question, Requirement, Resolution, Rule, Scope,
    ScopeId, Source, Topic, VerificationBinding, SUPPORTED_SCHEMA_VERSION,
};
use provenance_macros::rule;
use serde::{Deserialize, Serialize};

/// The families that travel in a pinned graph.
///
/// A pinned graph carries the canonical families and nothing else. The
/// collaboration and ideation records the repository also holds (proposals,
/// assertions, dispositions, contributions, synthesis packets, threads) say
/// who was talking and what they were still arguing about; they are not what
/// the graph asserts, and they never enter the projection.
///
/// This field list is the rule, not a check laid over it. There is no
/// predicate to run and nothing to remember: a family with no field here
/// cannot leave (`load_projection` has nowhere to put it) and cannot come
/// back (`deny_unknown_fields` refuses a document that names it). Adding a
/// field is how the rule changes; there is no other way to break it.
///
/// Deleting a field works the same way in reverse. Services and service
/// bindings once travelled here; with their fields gone, a pinned graph that
/// names them is refused on the way in, like any other family the projection
/// has no room for.
#[rule("rule_pinned_graph_families")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphExport {
    pub schema_version: u32,
    pub scope: Scope,
    pub sources: Vec<Source>,
    pub domains: Vec<Domain>,
    pub requirements: Vec<Requirement>,
    pub boundaries: Vec<Boundary>,
    pub topics: Vec<Topic>,
    pub questions: Vec<Question>,
    pub resolutions: Vec<Resolution>,
    pub rules: Vec<Rule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_bindings: Vec<VerificationBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub implementation_bindings: Vec<ImplementationBinding>,
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
        sources: store.closed_sources(&scope_id).map_err(incomplete)?,
        domains: store.closed_domains(&scope_id).map_err(incomplete)?,
        requirements: store.closed_requirements(&scope_id).map_err(incomplete)?,
        boundaries: store.closed_boundaries(&scope_id).map_err(incomplete)?,
        topics: store.closed_topics(&scope_id).map_err(incomplete)?,
        questions: store.closed_questions(&scope_id).map_err(incomplete)?,
        resolutions: store.closed_resolutions(&scope_id).map_err(incomplete)?,
        rules: store.closed_rules(&scope_id).map_err(incomplete)?,
        verification_bindings: store
            .closed_verification_bindings(&scope_id)
            .map_err(incomplete)?,
        implementation_bindings: store
            .closed_implementation_bindings(&scope_id)
            .map_err(incomplete)?,
    };
    graph.validate_schema_versions()?;
    validate_scope_ownership(&graph, &scope_id)?;
    strip_collaboration_fields(&mut graph);
    sort_records(&mut graph);
    Ok(graph)
}

impl GraphExport {
    pub(super) fn validate_schema_versions(&self) -> Result<(), GraphReferenceError> {
        macro_rules! require_supported {
            ($records:expr, $kind:literal) => {
                for record in $records {
                    ensure_graph_schema_version(
                        &format!("{} '{}'", $kind, record.id.as_str()),
                        record.schema_version,
                    )?;
                }
            };
        }
        require_supported!(&self.sources, "source");
        require_supported!(&self.domains, "domain");
        require_supported!(&self.requirements, "requirement");
        require_supported!(&self.boundaries, "boundary");
        require_supported!(&self.topics, "topic");
        require_supported!(&self.questions, "question");
        require_supported!(&self.resolutions, "resolution");
        require_supported!(&self.rules, "rule");
        require_supported!(&self.verification_bindings, "verification binding");
        require_supported!(&self.implementation_bindings, "implementation binding");
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
/// conversations, and they must not travel with the graph.
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

fn sort_records(graph: &mut GraphExport) {
    graph
        .sources
        .sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    graph
        .domains
        .sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    graph
        .requirements
        .sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    graph
        .boundaries
        .sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    graph
        .topics
        .sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    graph
        .questions
        .sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    graph
        .resolutions
        .sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    graph.rules.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    graph
        .verification_bindings
        .sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    graph
        .implementation_bindings
        .sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
}

/// Decides whether a pinned graph may travel as the scope it claims to be.
///
/// A pinned graph names one scope, and everything downstream reads its digest,
/// its record counts, and its contents as facts about that scope alone. A
/// single record carrying a different scope id would smuggle another scope's
/// data into the handover and make those counts lie, so every record of every
/// family must carry the claimed scope id. The first record that does not is
/// named in the refusal. Both directions are guarded here: what the store
/// hands out and what an exact export brings back in.
#[rule("rule_pinned_scope_ownership")]
pub(super) fn validate_scope_ownership(
    graph: &GraphExport,
    scope: &ScopeId,
) -> Result<(), GraphReferenceError> {
    macro_rules! require_scope {
        ($records:expr, $kind:literal) => {
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
    require_scope!(&graph.sources, "source");
    require_scope!(&graph.domains, "domain");
    require_scope!(&graph.requirements, "requirement");
    require_scope!(&graph.boundaries, "boundary");
    require_scope!(&graph.topics, "topic");
    require_scope!(&graph.questions, "question");
    require_scope!(&graph.resolutions, "resolution");
    require_scope!(&graph.rules, "rule");
    require_scope!(&graph.verification_bindings, "verification binding");
    require_scope!(&graph.implementation_bindings, "implementation binding");
    Ok(())
}

fn incomplete(error: impl std::fmt::Display) -> GraphReferenceError {
    GraphReferenceError::Incomplete {
        detail: error.to_string(),
    }
}

#[cfg(test)]
mod tests;
