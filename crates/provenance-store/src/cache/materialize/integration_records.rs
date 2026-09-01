use crate::state_store::StateStore;
use provenance_core::ScopeId;
use sqlx::{Sqlite, Transaction};

pub(super) async fn load_scope(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let mut loaded = 0;
    for binding in store.list_implementation_bindings(scope)? {
        sqlx::query("INSERT INTO implementation_bindings (scope_id, id, rule_id, declared_by, retired, file, symbol) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(binding.scope_id.as_str()).bind(binding.id.as_str()).bind(binding.rule_id.as_str())
            .bind(binding.declared_by).bind(binding.retired).bind(binding.file.as_str())
            .bind(binding.symbol).execute(&mut **tx).await?;
        loaded += 1;
    }
    for binding in store.list_verification_bindings(scope)? {
        sqlx::query("INSERT INTO verification_bindings (scope_id, id, rule_id, key, method, declared_by, retired, file, symbol) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(binding.scope_id.as_str()).bind(binding.id.as_str()).bind(binding.rule_id.as_str())
            .bind(binding.key).bind(crate::cache::serde_name(&binding.method)?)
            .bind(binding.declared_by).bind(binding.retired).bind(binding.file.as_str())
            .bind(binding.symbol).execute(&mut **tx).await?;
        loaded += 1;
    }
    for review in store.list_requirement_reviews(scope)? {
        sqlx::query("INSERT INTO requirement_reviews (scope_id, id, rule_id, requirement_id, field, before_text, after_text, changed_at, cleared_at, cleared_by_run) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(review.scope_id.as_str()).bind(review.id.as_str()).bind(review.rule_id.as_str())
            .bind(review.requirement_id.as_str()).bind(review.field).bind(review.before)
            .bind(review.after).bind(review.changed_at).bind(review.cleared_at)
            .bind(review.cleared_by_run.as_ref().map(provenance_core::StableId::as_str))
            .execute(&mut **tx).await?;
        loaded += 1;
    }
    Ok(loaded)
}
