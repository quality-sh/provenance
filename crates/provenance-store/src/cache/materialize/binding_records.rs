use crate::{cache::serde_name, state_store::StateStore};
use provenance_core::ScopeId;
use sqlx::{Sqlite, Transaction};

/// One row loader per binding and review family, keyed by
/// `PROJECTION_FAMILIES` name. Migration 018 added these tables.
pub(super) async fn load_implementation_bindings(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for binding in store.list_implementation_bindings(scope)? {
        sqlx::query(
            "INSERT INTO implementation_bindings (scope_id, id, rule_id, declared_by, retired, file, symbol, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(binding.scope_id.as_str())
        .bind(binding.id.as_str())
        .bind(binding.rule_id.as_str())
        .bind(&binding.declared_by)
        .bind(binding.retired)
        .bind(binding.file.as_str())
        .bind(&binding.symbol)
        .bind(serde_json::to_string(&binding)?)
        .execute(&mut **tx)
        .await?;
        loaded += 1;
    }
    Ok(loaded)
}

pub(super) async fn load_verification_bindings(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for binding in store.list_verification_bindings(scope)? {
        sqlx::query(
            "INSERT INTO verification_bindings (scope_id, id, rule_id, key, method, declared_by, retired, file, symbol, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(binding.scope_id.as_str())
        .bind(binding.id.as_str())
        .bind(binding.rule_id.as_str())
        .bind(&binding.key)
        .bind(serde_name(&binding.method)?)
        .bind(&binding.declared_by)
        .bind(binding.retired)
        .bind(binding.file.as_str())
        .bind(binding.symbol.as_deref())
        .bind(serde_json::to_string(&binding)?)
        .execute(&mut **tx)
        .await?;
        loaded += 1;
    }
    Ok(loaded)
}

pub(super) async fn load_requirement_reviews(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for review in store.open_requirement_reviews(scope)? {
        sqlx::query(
            "INSERT INTO requirement_reviews (scope_id, id, rule_id, requirement_id, field, changed_at, cleared_at, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(review.scope_id.as_str())
        .bind(review.id.as_str())
        .bind(review.rule_id.as_str())
        .bind(review.requirement_id.as_str())
        .bind(&review.field)
        .bind(review.changed_at)
        .bind(review.cleared_at)
        .bind(serde_json::to_string(&review)?)
        .execute(&mut **tx)
        .await?;
        loaded += 1;
    }
    Ok(loaded)
}
