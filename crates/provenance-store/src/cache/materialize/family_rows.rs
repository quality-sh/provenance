//! Per-family row replacement.
//!
//! Catch-up deletes one (family, scope) and reloads it through the same
//! loaders a full rebuild uses, so the two paths cannot derive rows
//! differently.

use super::{collaboration_records, graph_records, integration_records};
use crate::cache::ProjectionFamily;
use crate::state_store::StateStore;
use provenance_core::ScopeId;
use sqlx::{Sqlite, Transaction};

pub(super) async fn delete_rows(
    tx: &mut Transaction<'_, Sqlite>,
    family: ProjectionFamily,
    scope: &ScopeId,
) -> anyhow::Result<()> {
    sqlx::query(&format!(
        "DELETE FROM {} WHERE scope_id = ?",
        family.family_name()
    ))
    .bind(scope.as_str())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn load_rows(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    family: ProjectionFamily,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    match family {
        ProjectionFamily::Sources => graph_records::load_sources(tx, store, scope).await,
        ProjectionFamily::Domains => graph_records::load_domains(tx, store, scope).await,
        ProjectionFamily::Requirements => graph_records::load_requirements(tx, store, scope).await,
        ProjectionFamily::Boundaries => graph_records::load_boundaries(tx, store, scope).await,
        ProjectionFamily::Topics => graph_records::load_topics(tx, store, scope).await,
        ProjectionFamily::Questions => graph_records::load_questions(tx, store, scope).await,
        ProjectionFamily::Resolutions => graph_records::load_resolutions(tx, store, scope).await,
        ProjectionFamily::Rules => graph_records::load_rules(tx, store, scope).await,
        ProjectionFamily::Threads => collaboration_records::load_threads(tx, store, scope).await,
        ProjectionFamily::Messages => collaboration_records::load_messages(tx, store, scope).await,
        ProjectionFamily::Contributions => {
            collaboration_records::load_contributions(tx, store, scope).await
        }
        ProjectionFamily::SynthesisPackets => {
            collaboration_records::load_synthesis_packets(tx, store, scope).await
        }
        ProjectionFamily::AssertionRecords => {
            collaboration_records::load_assertion_records(tx, store, scope).await
        }
        ProjectionFamily::ProposalCards => {
            collaboration_records::load_proposal_cards(tx, store, scope).await
        }
        ProjectionFamily::Dispositions => {
            collaboration_records::load_dispositions(tx, store, scope).await
        }
        ProjectionFamily::ImplementationBindings => {
            integration_records::load_implementation_bindings(tx, store, scope).await
        }
        ProjectionFamily::VerificationBindings => {
            integration_records::load_verification_bindings(tx, store, scope).await
        }
        ProjectionFamily::RequirementReviews => {
            integration_records::load_requirement_reviews(tx, store, scope).await
        }
    }
}
