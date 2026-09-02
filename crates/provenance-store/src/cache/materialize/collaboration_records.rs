use crate::{cache::serde_name, state_store::StateStore};
use provenance_core::ScopeId;
use sqlx::{Sqlite, Transaction};

pub(super) async fn load_scope(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = load_threads(tx, store, scope).await?;
    loaded += load_messages(tx, store, scope).await?;
    loaded += load_contributions(tx, store, scope).await?;
    loaded += load_synthesis_packets(tx, store, scope).await?;
    loaded += load_assertion_records(tx, store, scope).await?;
    loaded += load_proposal_cards(tx, store, scope).await?;
    loaded += load_dispositions(tx, store, scope).await?;
    Ok(loaded)
}

pub(super) async fn load_threads(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for thread in store.list_threads(scope)? {
        sqlx::query("INSERT INTO threads (scope_id, id, parent_type, parent_id, status, created_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(thread.scope_id.as_str()).bind(thread.id.as_str())
            .bind(serde_name(&thread.parent.node_type)?).bind(thread.parent.node_id.as_str())
            .bind(serde_name(&thread.status)?).bind(thread.created_at)
            .execute(&mut **tx).await?;
        loaded += 1;
    }
    Ok(loaded)
}

pub(super) async fn load_messages(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for message in store.list_messages(scope)? {
        sqlx::query("INSERT INTO messages (scope_id, id, thread_id, role, body, created_at, ai_metadata) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(message.scope_id.as_str()).bind(message.id.as_str()).bind(message.thread_id.as_str())
            .bind(serde_name(&message.role)?).bind(message.body).bind(message.created_at)
            .bind(message.ai_metadata.map(|value| value.to_string()))
            .execute(&mut **tx).await?;
        loaded += 1;
    }
    Ok(loaded)
}

pub(super) async fn load_contributions(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for contribution in store.list_contributions(scope)? {
        sqlx::query("INSERT INTO contributions (scope_id, id, target_type, target_id, participant_slot, stance, strongest_finding, uncertainty, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(contribution.scope_id.as_str()).bind(contribution.id.as_str())
            .bind(serde_name(&contribution.target.artifact_type)?)
            .bind(contribution.target.artifact_id.as_str()).bind(&contribution.participant_slot)
            .bind(serde_name(&contribution.stance)?).bind(&contribution.strongest_finding)
            .bind(serde_json::to_string(&contribution.uncertainty)?)
            .bind(serde_json::to_string(&contribution)?).execute(&mut **tx).await?;
        loaded += 1;
    }
    Ok(loaded)
}

pub(super) async fn load_synthesis_packets(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for packet in store.list_synthesis_packets(scope)? {
        sqlx::query("INSERT INTO synthesis_packets (scope_id, id, target_type, target_id, summary, payload) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(packet.scope_id.as_str()).bind(packet.id.as_str())
            .bind(serde_name(&packet.target.artifact_type)?).bind(packet.target.artifact_id.as_str())
            .bind(&packet.summary).bind(serde_json::to_string(&packet)?)
            .execute(&mut **tx).await?;
        loaded += 1;
    }
    Ok(loaded)
}

pub(super) async fn load_assertion_records(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for assertion in store.list_assertion_records(scope)? {
        sqlx::query("INSERT INTO assertion_records (scope_id, id, proposal_id, synthesis_packet_id, supporting_claim_ids, payload) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(assertion.scope_id.as_str()).bind(assertion.id.as_str())
            .bind(assertion.proposal_id.as_str()).bind(assertion.synthesis_packet_id.as_str())
            .bind(serde_json::to_string(&assertion.supporting_claim_ids)?)
            .bind(serde_json::to_string(&assertion)?).execute(&mut **tx).await?;
        loaded += 1;
    }
    Ok(loaded)
}

pub(super) async fn load_proposal_cards(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for proposal in store.list_proposal_cards(scope)? {
        sqlx::query("INSERT INTO proposal_cards (scope_id, id, proposal_key, proposal_type, title, summary, confidence, target_type, target_id, traceability, builds_on, promotion_state, duplicate_of, superseded_by) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(proposal.scope_id.as_str()).bind(proposal.id.as_str()).bind(&proposal.proposal_key)
            .bind(serde_name(&proposal.proposal_type)?).bind(&proposal.title).bind(&proposal.summary)
            .bind(proposal.confidence).bind(serde_name(&proposal.traceability.target.artifact_type)?)
            .bind(proposal.traceability.target.artifact_id.as_str())
            .bind(serde_json::to_string(&proposal.traceability)?)
            .bind(serde_json::to_string(&proposal.builds_on)?)
            .bind(serde_name(&proposal.promotion_state)?)
            .bind(proposal.duplicate_of.as_ref().map(provenance_core::StableId::as_str))
            .bind(proposal.superseded_by.as_ref().map(provenance_core::StableId::as_str))
            .execute(&mut **tx).await?;
        loaded += 1;
    }
    Ok(loaded)
}

pub(super) async fn load_dispositions(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for disposition in store.list_dispositions(scope)? {
        sqlx::query("INSERT INTO dispositions (scope_id, id, proposal_id, decision, rationale, actor, canonical_artifact, external_action) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(disposition.scope_id.as_str()).bind(disposition.id.as_str()).bind(disposition.proposal_id.as_str())
            .bind(serde_name(&disposition.decision)?).bind(&disposition.rationale)
            .bind(serde_json::to_string(&disposition.actor)?)
            .bind(disposition.canonical_artifact.as_ref().map(serde_json::to_string).transpose()?)
            .bind(disposition.external_action.as_ref().map(serde_json::to_string).transpose()?)
            .execute(&mut **tx).await?;
        loaded += 1;
    }
    Ok(loaded)
}
