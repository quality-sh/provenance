use super::{binding_records, collaboration_records, graph_records};
use crate::cache::projection_families::ProjectionFamily;
use crate::state_store::StateStore;
use provenance_core::ScopeId;
use sqlx::{Sqlite, Transaction};

/// Removes every row of one family's table.
pub(super) async fn clear_family(
    tx: &mut Transaction<'_, Sqlite>,
    family: &ProjectionFamily,
) -> anyhow::Result<()> {
    sqlx::query(&format!("DELETE FROM {}", family.table))
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Removes one scope's rows of one family's table.
pub(super) async fn clear_family_scope(
    tx: &mut Transaction<'_, Sqlite>,
    family: &ProjectionFamily,
    scope: &ScopeId,
) -> anyhow::Result<()> {
    sqlx::query(&format!("DELETE FROM {} WHERE scope_id = ?", family.table))
        .bind(scope.as_str())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Re-derives one family's rows for one scope from the canonical readers.
///
/// Row content always comes from the `StateStore` over canonical bytes, so
/// a stale journal event can never inject a row canonical does not hold.
/// The global edges family is derived once for all scopes.
pub(super) async fn load_family(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    family: &ProjectionFamily,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    match family.name {
        "sources" => graph_records::load_sources(tx, store, scope).await,
        "domains" => graph_records::load_domains(tx, store, scope).await,
        "requirements" => graph_records::load_requirements(tx, store, scope).await,
        "boundaries" => graph_records::load_boundaries(tx, store, scope).await,
        "topics" => graph_records::load_topics(tx, store, scope).await,
        "questions" => graph_records::load_questions(tx, store, scope).await,
        "edges" => graph_records::load_all_edges(tx, store).await,
        "resolutions" => graph_records::load_resolutions(tx, store, scope).await,
        "rules" => graph_records::load_rules(tx, store, scope).await,
        "messages" => collaboration_records::load_messages(tx, store, scope).await,
        "threads" => collaboration_records::load_threads(tx, store, scope).await,
        "contributions" => collaboration_records::load_contributions(tx, store, scope).await,
        "synthesis_packets" => {
            collaboration_records::load_synthesis_packets(tx, store, scope).await
        }
        "proposal_cards" => collaboration_records::load_proposal_cards(tx, store, scope).await,
        "assertion_records" => {
            collaboration_records::load_assertion_records(tx, store, scope).await
        }
        "dispositions" => collaboration_records::load_dispositions(tx, store, scope).await,
        "implementation_bindings" => {
            binding_records::load_implementation_bindings(tx, store, scope).await
        }
        "verification_bindings" => {
            binding_records::load_verification_bindings(tx, store, scope).await
        }
        "requirement_reviews" => binding_records::load_requirement_reviews(tx, store, scope).await,
        _ => anyhow::bail!("projection family '{}' has no row loader", family.name),
    }
}

/// Every family name the dispatch above can derive. The loader coverage
/// test walks this against `PROJECTION_FAMILIES`, so a family added to
/// the table without a loader fails the suite.
#[cfg(test)]
pub(super) fn loader_names() -> Vec<&'static str> {
    vec![
        "sources",
        "domains",
        "requirements",
        "boundaries",
        "topics",
        "questions",
        "edges",
        "resolutions",
        "rules",
        "messages",
        "threads",
        "contributions",
        "synthesis_packets",
        "proposal_cards",
        "assertion_records",
        "dispositions",
        "implementation_bindings",
        "verification_bindings",
        "requirement_reviews",
    ]
}
