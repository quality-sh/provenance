use std::collections::BTreeSet;

use provenance_core::{DeclarationAddress, EdgeType, NodeType, ScopeId, StableId};

use super::{rule_address, DesiredTypedGraph};
use crate::state_store::StateStore;

pub(super) fn reconcile(
    store: &StateStore,
    scope_id: &ScopeId,
    graph: DesiredTypedGraph<'_>,
) -> anyhow::Result<()> {
    let references = desired_references(graph);
    let produces = desired_produces(graph)?;
    remove_superseded_edges(store, scope_id, graph, &references, &produces)?;
    for (source, requirement) in references {
        store.add_edge(
            scope_id.clone(),
            EdgeType::References,
            NodeType::Source,
            StableId::new(source)?,
            NodeType::Requirement,
            StableId::new(requirement)?,
        )?;
    }
    for (requirement, rule) in produces {
        store.add_edge(
            scope_id.clone(),
            EdgeType::Produces,
            NodeType::Requirement,
            StableId::new(requirement)?,
            NodeType::Rule,
            StableId::new(rule)?,
        )?;
    }
    Ok(())
}

fn desired_references(graph: DesiredTypedGraph<'_>) -> BTreeSet<(String, String)> {
    graph
        .requirements
        .iter()
        .flat_map(|declaration| {
            declaration.sources.iter().map(|source| {
                (
                    graph.source_ids[source].as_str().to_string(),
                    graph.requirement_ids[&declaration.key].as_str().to_string(),
                )
            })
        })
        .collect()
}

fn desired_produces(graph: DesiredTypedGraph<'_>) -> anyhow::Result<BTreeSet<(String, String)>> {
    let mut relationships = BTreeSet::new();
    for declaration in graph.rules {
        let address = rule_address(graph.spec, declaration)?;
        for requirement in &declaration.requirements {
            relationships.insert((
                graph.requirement_ids[requirement].as_str().to_string(),
                graph.rule_ids[&address].as_str().to_string(),
            ));
        }
    }
    Ok(relationships)
}

fn remove_superseded_edges(
    store: &StateStore,
    scope_id: &ScopeId,
    graph: DesiredTypedGraph<'_>,
    references: &BTreeSet<(String, String)>,
    produces: &BTreeSet<(String, String)>,
) -> anyhow::Result<()> {
    let managed_sources = managed_ids(
        store.list_sources(scope_id)?.iter().map(|record| {
            (
                &record.id,
                record.declared_by.as_deref(),
                record.declaration_address.as_ref(),
            )
        }),
        graph,
    );
    let managed_requirements = managed_ids(
        store.list_requirements(scope_id)?.iter().map(|record| {
            (
                &record.id,
                record.declared_by.as_deref(),
                record.declaration_address.as_ref(),
            )
        }),
        graph,
    );
    let desired_requirements = graph
        .requirement_ids
        .values()
        .map(|id| id.as_str().to_string())
        .collect::<BTreeSet<_>>();
    let desired_rules = graph
        .rule_ids
        .values()
        .map(|id| id.as_str().to_string())
        .collect::<BTreeSet<_>>();

    let stale = store
        .list_edges()?
        .into_iter()
        .filter(|edge| edge.scope_id == *scope_id)
        .filter(|edge| match edge.edge_type {
            EdgeType::References => {
                edge.from_type == NodeType::Source
                    && edge.to_type == NodeType::Requirement
                    && managed_sources.contains(edge.from_id.as_str())
                    && desired_requirements.contains(edge.to_id.as_str())
                    && !references.contains(&(
                        edge.from_id.as_str().to_string(),
                        edge.to_id.as_str().to_string(),
                    ))
            }
            EdgeType::Produces => {
                edge.from_type == NodeType::Requirement
                    && edge.to_type == NodeType::Rule
                    && managed_requirements.contains(edge.from_id.as_str())
                    && desired_rules.contains(edge.to_id.as_str())
                    && !produces.contains(&(
                        edge.from_id.as_str().to_string(),
                        edge.to_id.as_str().to_string(),
                    ))
            }
            EdgeType::RefinesInto
            | EdgeType::DependsOn
            | EdgeType::Contradicts
            | EdgeType::Supersedes
            | EdgeType::Needs
            | EdgeType::Resolves
            | EdgeType::Spawns => false,
        })
        .map(|edge| edge.id)
        .collect::<Vec<_>>();
    for id in stale {
        store.delete_edge(scope_id, &id)?;
    }
    Ok(())
}

fn managed_ids<'a>(
    records: impl Iterator<
        Item = (
            &'a StableId,
            Option<&'a str>,
            Option<&'a DeclarationAddress>,
        ),
    >,
    graph: DesiredTypedGraph<'_>,
) -> BTreeSet<String> {
    records
        .filter(|(_, owner, address)| {
            *owner == Some(graph.owner)
                && address.is_some_and(|address| {
                    address
                        .segments()
                        .first()
                        .is_some_and(|part| part == graph.spec)
                })
        })
        .map(|(id, _, _)| id.as_str().to_string())
        .collect()
}
