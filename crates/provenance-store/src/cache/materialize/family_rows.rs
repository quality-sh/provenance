//! Per-family row replacement.
//!
//! A full rebuild and a catch-up pass both write one (family, scope)
//! through here, so the two paths cannot derive rows differently. The
//! eleven record families go through the one `ProjectionRow` loader; the
//! seven collaboration families keep their hand-written inserts.

use super::record_rows::{kind_search, load_kind};
use super::{collaboration_records, record_rows};
use crate::cache::ProjectionFamily;
use crate::state_store::StateStore;
use provenance_core::protocol::GraphNode;
use provenance_core::ScopeId;
use sqlx::{Sqlite, Transaction};

pub(super) async fn delete_rows(
    tx: &mut Transaction<'_, Sqlite>,
    family: ProjectionFamily,
    scope: &ScopeId,
) -> anyhow::Result<()> {
    sqlx::query(&format!(
        "DELETE FROM {} WHERE scope_id = ?",
        record_rows::quoted(family.family_name())
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
        ProjectionFamily::Sources => {
            let search = kind_search(GraphNode::Source);
            load_kind(tx, store.list_sources(scope)?, Some(&search)).await
        }
        ProjectionFamily::Domains => {
            let search = kind_search(GraphNode::Domain);
            load_kind(tx, store.list_domains(scope)?, Some(&search)).await
        }
        ProjectionFamily::Requirements => {
            let search = kind_search(GraphNode::Requirement);
            load_kind(tx, store.list_requirements(scope)?, Some(&search)).await
        }
        ProjectionFamily::Boundaries => {
            let search = kind_search(GraphNode::Boundary);
            load_kind(tx, store.list_boundaries(scope)?, Some(&search)).await
        }
        ProjectionFamily::Topics => {
            let search = kind_search(GraphNode::Topic);
            load_kind(tx, store.list_topics(scope)?, Some(&search)).await
        }
        ProjectionFamily::Questions => {
            let search = kind_search(GraphNode::Question);
            load_kind(tx, store.list_questions(scope)?, Some(&search)).await
        }
        ProjectionFamily::Resolutions => {
            let search = kind_search(GraphNode::Resolution);
            load_kind(tx, store.list_resolutions(scope)?, Some(&search)).await
        }
        ProjectionFamily::Rules => {
            let search = kind_search(GraphNode::Rule);
            load_kind(tx, store.list_rules(scope)?, Some(&search)).await
        }
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
            load_kind(tx, store.list_implementation_bindings(scope)?, None).await
        }
        ProjectionFamily::VerificationBindings => {
            load_kind(tx, store.list_verification_bindings(scope)?, None).await
        }
        ProjectionFamily::RequirementReviews => {
            load_kind(tx, store.list_requirement_reviews(scope)?, None).await
        }
    }
}
