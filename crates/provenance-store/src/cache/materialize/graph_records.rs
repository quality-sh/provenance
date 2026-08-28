use crate::{cache::serde_name, state_store::StateStore};
use provenance_core::ScopeId;
use sqlx::{Sqlite, Transaction};

/// One row loader per graph-family table, keyed by
/// `PROJECTION_FAMILIES` name. Rebuild and catch-up re-derivation both
/// call these, so rows always come from the same reader path.
pub(super) async fn load_sources(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for source in store.list_sources(scope)? {
        let payload = serde_json::to_string(&source)?;
        sqlx::query("INSERT INTO sources (scope_id, id, name, source_type, url, reference, commit_pin, effective_date, review_date, superseded_by, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(source.scope_id.as_str()).bind(source.id.as_str()).bind(source.name)
            .bind(serde_name(&source.source_type)?).bind(source.url).bind(source.reference)
            .bind(source.commit_pin).bind(source.effective_date).bind(source.review_date)
            .bind(source.superseded_by.as_ref().map(provenance_core::StableId::as_str))
            .bind(payload)
            .execute(&mut **tx).await?;
        loaded += 1;
    }
    Ok(loaded)
}

pub(super) async fn load_requirements(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for requirement in store.list_requirements(scope)? {
        let payload = serde_json::to_string(&requirement)?;
        sqlx::query("INSERT INTO requirements (scope_id, id, statement, status, domain_id, fog, payload) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(requirement.scope_id.as_str()).bind(requirement.id.as_str())
            .bind(requirement.statement).bind(serde_name(&requirement.status)?)
            .bind(requirement.domain_id.as_ref().map(provenance_core::StableId::as_str))
            .bind(requirement.fog)
            .bind(payload)
            .execute(&mut **tx).await?;
        loaded += 1;
    }
    Ok(loaded)
}

pub(super) async fn load_domains(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for domain in store.list_domains(scope)? {
        let payload = serde_json::to_string(&domain)?;
        sqlx::query(
            "INSERT INTO domains (scope_id, id, name, description, color, payload) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(domain.scope_id.as_str())
        .bind(domain.id.as_str())
        .bind(domain.name)
        .bind(domain.description)
        .bind(domain.color)
        .bind(payload)
        .execute(&mut **tx)
        .await?;
        loaded += 1;
    }
    Ok(loaded)
}

pub(super) async fn load_boundaries(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for boundary in store.list_boundaries(scope)? {
        let payload = serde_json::to_string(&boundary)?;
        let source_id = boundary
            .source_ref
            .as_ref()
            .map(|reference| reference.source_id.as_str());
        let source_clause = boundary
            .source_ref
            .as_ref()
            .and_then(|reference| reference.clause.as_deref());
        sqlx::query("INSERT INTO boundaries (scope_id, id, requirement_id, statement, source_id, source_clause, payload) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(boundary.scope_id.as_str()).bind(boundary.id.as_str())
            .bind(boundary.requirement_id.as_str()).bind(boundary.statement)
            .bind(source_id).bind(source_clause)
            .bind(payload)
            .execute(&mut **tx).await?;
        loaded += 1;
    }
    Ok(loaded)
}

pub(super) async fn load_topics(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for topic in store.list_topics(scope)? {
        let payload = serde_json::to_string(&topic)?;
        sqlx::query("INSERT INTO topics (scope_id, id, requirement_id, title, status, claimed_by, claimed_at, links, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(topic.scope_id.as_str()).bind(topic.id.as_str()).bind(topic.requirement_id.as_str())
            .bind(topic.title).bind(serde_name(&topic.status)?).bind(topic.claimed_by)
            .bind(topic.claimed_at).bind(serde_json::to_string(&topic.links)?)
            .bind(payload)
            .execute(&mut **tx).await?;
        loaded += 1;
    }
    Ok(loaded)
}

pub(super) async fn load_questions(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for question in store.list_questions(scope)? {
        let payload = serde_json::to_string(&question)?;
        sqlx::query("INSERT INTO questions (scope_id, id, topic_id, requirement_id, question, resolution_method, status, claimed_by, claimed_at, answer, links, resolution_id, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(question.scope_id.as_str()).bind(question.id.as_str()).bind(question.topic_id.as_str())
            .bind(question.requirement_id.as_str()).bind(question.question)
            .bind(serde_name(&question.resolution_method)?).bind(serde_name(&question.status)?)
            .bind(question.claimed_by).bind(question.claimed_at).bind(question.answer)
            .bind(serde_json::to_string(&question.links)?)
            .bind(question.resolution_id.as_ref().map(provenance_core::StableId::as_str))
            .bind(payload)
            .execute(&mut **tx).await?;
        loaded += 1;
    }
    Ok(loaded)
}

pub(super) async fn load_resolutions(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for resolution in store.list_resolutions(scope)? {
        let payload = serde_json::to_string(&resolution)?;
        sqlx::query("INSERT INTO resolutions (scope_id, id, title, position, rationale, status, review_on, context, enforcement, confidence, inputs, made_by, approved_by, approved_at, superseded_by, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(resolution.scope_id.as_str()).bind(resolution.id.as_str()).bind(resolution.title)
            .bind(resolution.position).bind(resolution.rationale).bind(serde_name(&resolution.status)?)
            .bind(resolution.review_on)
            .bind(resolution.context).bind(resolution.enforcement).bind(resolution.confidence)
            .bind(serde_json::to_string(&resolution.inputs)?).bind(resolution.made_by)
            .bind(resolution.approved_by).bind(resolution.approved_at)
            .bind(resolution.superseded_by.as_ref().map(provenance_core::StableId::as_str))
            .bind(payload)
            .execute(&mut **tx).await?;
        loaded += 1;
    }
    Ok(loaded)
}

pub(super) async fn load_rules(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for rule in store.list_rules(scope)? {
        let payload = serde_json::to_string(&rule)?;
        sqlx::query(
            "INSERT INTO rules (scope_id, id, statement, status, severity, payload) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(rule.scope_id.as_str())
        .bind(rule.id.as_str())
        .bind(rule.statement)
        .bind(serde_name(&rule.status)?)
        .bind(serde_name(&rule.severity)?)
        .bind(payload)
        .execute(&mut **tx)
        .await?;
        loaded += 1;
    }
    Ok(loaded)
}

pub(super) async fn load_all_edges(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for edge in store.list_edges()? {
        let payload = serde_json::to_string(&edge)?;
        sqlx::query("INSERT INTO edges (scope_id, id, edge_type, from_type, from_id, to_type, to_id, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(edge.scope_id.as_str()).bind(edge.id.as_str()).bind(serde_name(&edge.edge_type)?)
            .bind(serde_name(&edge.from_type)?).bind(edge.from_id.as_str())
            .bind(serde_name(&edge.to_type)?).bind(edge.to_id.as_str())
            .bind(payload)
            .execute(&mut **tx).await?;
        loaded += 1;
    }
    Ok(loaded)
}
