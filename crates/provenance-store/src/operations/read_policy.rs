//! The reader policy: freshness, budgets, and the journal knob.
//!
//! One module owns `read.freshness_policy` (`catch_up` by default,
//! `annotate_only` for offline stamping, `refuse_stale` for typed
//! refusal), the visit and scan budget defaults, and the
//! `cache.catchup_journal` switch. The served read path takes the
//! publication guard once here, runs the freshness step, and answers from
//! the stamped snapshot, so reversal touches one site.

use crate::layout::ProvenanceLayout;
use crate::{cache, publication};
use provenance_core::protocol::{
    AttestedDomain, FreshnessPolicy, FreshnessStamp, LiveConstituent, SCAN_BUDGET_DEFAULT,
    VISIT_BUDGET_DEFAULT,
};
use serde::Deserialize;
use sqlx::Row;

/// Configuration the repository persists beside canonical state.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RepositoryConfig {
    #[serde(default)]
    pub read: ReadConfig,
    #[serde(default)]
    pub cache: CacheConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadConfig {
    #[serde(default)]
    pub freshness_policy: FreshnessPolicy,
    #[serde(default = "default_visit_budget")]
    pub visit_budget: usize,
    #[serde(default = "default_scan_budget")]
    pub scan_budget: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheConfig {
    /// Whether writers record invalidation events. The journal is a work
    /// hint, never a freshness proof: every full-freshness catch-up hashes
    /// every family with the journal on or off.
    #[serde(default = "default_true")]
    pub catchup_journal: bool,
}

impl Default for ReadConfig {
    fn default() -> Self {
        Self {
            freshness_policy: FreshnessPolicy::CatchUp,
            visit_budget: VISIT_BUDGET_DEFAULT,
            scan_budget: SCAN_BUDGET_DEFAULT,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            catchup_journal: true,
        }
    }
}

const fn default_visit_budget() -> usize {
    VISIT_BUDGET_DEFAULT
}

const fn default_scan_budget() -> usize {
    SCAN_BUDGET_DEFAULT
}

const fn default_true() -> bool {
    true
}

impl RepositoryConfig {
    /// Loads the repository configuration, falling back to defaults when
    /// no configuration file exists.
    pub fn load(layout: &ProvenanceLayout) -> Self {
        let path = layout.provenance_dir().join("config.json");
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// The freshness policy the repository serves reads under.
    pub fn freshness_policy(layout: &ProvenanceLayout) -> FreshnessPolicy {
        Self::load(layout).read.freshness_policy
    }
}

/// The typed refusal `refuse_stale` returns when the projection cannot be
/// made current. Machine-readable: callers match on the type, not on text.
#[derive(Debug, thiserror::Error)]
#[error("staleness refused: the projection cannot be made current ({detail})")]
pub struct StalenessRefusal {
    pub detail: String,
}

/// What one stamped read answered behind.
pub struct StampedRead {
    pub stamp: Option<FreshnessStamp>,
}

/// Runs one served read under the repository policy.
///
/// `catch_up` materializes then serves locally; `annotate_only` stamps
/// from the stored revision without catching up; `refuse_stale` refuses
/// with a typed error when catch-up cannot make the stamp current. The
/// guard is taken once here and the caller answers inside the held scope,
/// so the answer is snapshot-consistent with the stamp. The freshness
/// report is handed to the answer closure by value.
pub async fn stamped_read<T, Fut>(
    layout: &ProvenanceLayout,
    attested: Vec<AttestedDomain>,
    live: Vec<LiveConstituent>,
    answer: impl FnOnce(
        std::sync::Arc<crate::publication::guard::PublicationGuard>,
        cache::CatchUpReport,
    ) -> Fut,
) -> anyhow::Result<(T, StampedRead)>
where
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let policy = RepositoryConfig::freshness_policy(layout);
    let guard = std::sync::Arc::new(publication::guard::publication_guard(layout).await?);
    let guard_handle = guard.clone();
    let fresh = match policy {
        FreshnessPolicy::AnnotateOnly => None,
        FreshnessPolicy::CatchUp | FreshnessPolicy::RefuseStale => Some(
            cache::catch_up_state_under_guard(&guard, layout)
                .await
                .map_err(|error| {
                    if policy == FreshnessPolicy::RefuseStale {
                        anyhow::Error::new(StalenessRefusal {
                            detail: error.to_string(),
                        })
                    } else {
                        error
                    }
                })?,
        ),
    };
    let stamp_report = fresh.clone();
    let answer_report = fresh.clone().unwrap_or_else(|| cache::CatchUpReport {
        rebuilt: false,
        families_rederived: Vec::new(),
        families_verified: 0,
        journal_drained: 0,
        serial: 0,
        digest: String::new(),
        instance_id: String::new(),
        migrations_applied: Vec::new(),
    });
    let answered = answer(guard_handle.clone(), answer_report).await?;
    let stamp = if let Some(fresh) = &stamp_report {
        Some(FreshnessStamp {
            instance: fresh.instance_id.clone(),
            serial: fresh.serial,
            digest: fresh.digest.clone(),
            policy,
            attested,
            live,
        })
    } else {
        let stored = stored_revision(layout).await?;
        stored.map(|stored| FreshnessStamp {
            instance: stored.instance_id,
            serial: stored.serial,
            digest: stored.digest,
            policy,
            attested,
            live,
        })
    };
    Ok((answered, StampedRead { stamp }))
}

/// Reads the stored revision without catching up.
async fn stored_revision(layout: &ProvenanceLayout) -> anyhow::Result<Option<StoredStamp>> {
    let pool = cache::open_cache(layout).await?;
    let row = sqlx::query("SELECT serial, instance_id, digest FROM projection_revision LIMIT 1")
        .fetch_optional(&pool)
        .await?;
    pool.close().await;
    Ok(row.map(|row| StoredStamp {
        serial: row.get::<i64, _>(0),
        instance_id: row.get::<String, _>(1),
        digest: row.get::<String, _>(2),
    }))
}

struct StoredStamp {
    serial: i64,
    instance_id: String,
    digest: String,
}

/// Guards journal emission on the repository knob, keeping the journal a
/// default-on work hint.
pub fn journal_enabled(layout: &ProvenanceLayout) -> bool {
    RepositoryConfig::load(layout).cache.catchup_journal
}
