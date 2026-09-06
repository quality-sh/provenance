//! Per-family row replacement.
//!
//! A full rebuild and a catch-up pass both write one (family, scope)
//! through here, so the two paths cannot derive rows differently. The
//! eleven record families go through the one `ProjectionRow` loader; the
//! seven collaboration families keep their hand-written inserts.

use super::collaboration_records;
use super::record_rows::{kind_search, load_kind};
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
    Ok(())
}

pub(super) async fn load_rows(
    tx: &mut Transaction<'_, Sqlite>,
    family: ProjectionFamily,
    bytes: &[u8],
) -> anyhow::Result<u64> {
    match family {
        ProjectionFamily::Sources => {
            let search = kind_search(GraphNode::Source);
            load_kind::<Source>(tx, bytes, Some(&search)).await
        }
        ProjectionFamily::Domains => {
            let search = kind_search(GraphNode::Domain);
            load_kind::<Domain>(tx, bytes, Some(&search)).await
        }
        ProjectionFamily::Requirements => {
            let search = kind_search(GraphNode::Requirement);
            load_kind::<Requirement>(tx, bytes, Some(&search)).await
        }
        ProjectionFamily::Boundaries => {
            let search = kind_search(GraphNode::Boundary);
            load_kind::<Boundary>(tx, bytes, Some(&search)).await
        }
        ProjectionFamily::Topics => {
            let search = kind_search(GraphNode::Topic);
            load_kind::<Topic>(tx, bytes, Some(&search)).await
        }
        ProjectionFamily::Questions => {
            let search = kind_search(GraphNode::Question);
            load_kind::<Question>(tx, bytes, Some(&search)).await
        }
        ProjectionFamily::Resolutions => {
            let search = kind_search(GraphNode::Resolution);
            load_kind::<Resolution>(tx, bytes, Some(&search)).await
        }
        ProjectionFamily::Rules => {
            let search = kind_search(GraphNode::Rule);
            load_kind::<Rule>(tx, bytes, Some(&search)).await
        }
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
            load_kind::<ImplementationBinding>(tx, bytes, None).await
        }
        ProjectionFamily::VerificationBindings => {
            load_kind::<VerificationBinding>(tx, bytes, None).await
        }
        ProjectionFamily::RequirementReviews => {
            load_kind::<RequirementReview>(tx, bytes, None).await
        }
    }
}
