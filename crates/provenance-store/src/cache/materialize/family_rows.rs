//! Per-family row replacement.
//!
//! A full rebuild and a catch-up pass both write one (family, scope)
//! through here, so the two paths cannot derive rows differently. The
//! eleven record families go through the one `ProjectionRow` loader; the
//! seven collaboration families keep their hand-written inserts.

use super::collaboration_records;
use super::record_rows::{load_kind, load_record};
use crate::cache::quoted;
use crate::cache::ProjectionFamily;
use provenance_core::protocol::GraphNode;
use provenance_core::{
    Boundary, Domain, ImplementationBinding, Question, Requirement, RequirementReview, Resolution,
    Rule, ScopeId, Source, Topic, VerificationBinding,
};
use sqlx::{Sqlite, Transaction};

pub(super) async fn delete_rows(
    tx: &mut Transaction<'_, Sqlite>,
    family: ProjectionFamily,
    scope: &ScopeId,
) -> anyhow::Result<()> {
    sqlx::query(&format!(
        "DELETE FROM {} WHERE scope_id = ?",
        quoted(family.family_name())
    ))
    .bind(scope.as_str())
    .execute(&mut **tx)
    .await?;
    if let Some(node_type) = family.node_type() {
        sqlx::query("DELETE FROM record_identities WHERE scope_id = ? AND node_type = ?")
            .bind(scope.as_str())
            .bind(node_type.as_str())
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

pub(super) async fn load_rows(
    tx: &mut Transaction<'_, Sqlite>,
    family: ProjectionFamily,
    bytes: &[u8],
) -> anyhow::Result<u64> {
    match family {
        ProjectionFamily::Sources => load_record::<Source>(tx, bytes, GraphNode::Source).await,
        ProjectionFamily::Domains => load_record::<Domain>(tx, bytes, GraphNode::Domain).await,
        ProjectionFamily::Requirements => {
            load_record::<Requirement>(tx, bytes, GraphNode::Requirement).await
        }
        ProjectionFamily::Boundaries => {
            load_record::<Boundary>(tx, bytes, GraphNode::Boundary).await
        }
        ProjectionFamily::Topics => load_record::<Topic>(tx, bytes, GraphNode::Topic).await,
        ProjectionFamily::Questions => {
            load_record::<Question>(tx, bytes, GraphNode::Question).await
        }
        ProjectionFamily::Resolutions => {
            load_record::<Resolution>(tx, bytes, GraphNode::Resolution).await
        }
        ProjectionFamily::Rules => load_record::<Rule>(tx, bytes, GraphNode::Rule).await,
        ProjectionFamily::Threads => collaboration_records::load_threads(tx, bytes).await,
        ProjectionFamily::Messages => collaboration_records::load_messages(tx, bytes).await,
        ProjectionFamily::Contributions => {
            collaboration_records::load_contributions(tx, bytes).await
        }
        ProjectionFamily::SynthesisPackets => {
            collaboration_records::load_synthesis_packets(tx, bytes).await
        }
        ProjectionFamily::AssertionRecords => {
            collaboration_records::load_assertion_records(tx, bytes).await
        }
        ProjectionFamily::ProposalCards => {
            collaboration_records::load_proposal_cards(tx, bytes).await
        }
        ProjectionFamily::Dispositions => collaboration_records::load_dispositions(tx, bytes).await,
        ProjectionFamily::ImplementationBindings => {
            load_kind::<ImplementationBinding>(tx, bytes).await
        }
        ProjectionFamily::VerificationBindings => load_kind::<VerificationBinding>(tx, bytes).await,
        ProjectionFamily::ReviewJournal => crate::review::cache::load_rows(tx, bytes).await,
        ProjectionFamily::RequirementReviews => load_kind::<RequirementReview>(tx, bytes).await,
    }
}
