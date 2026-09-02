use crate::layout::ProvenanceLayout;
use anyhow::Context;
use sqlx::{Executor, SqlitePool};

pub const INITIAL_MIGRATION_ID: &str = "001";
pub const SOURCE_REQUIREMENT_MIGRATION_ID: &str = "002";
pub const RESOLUTIONS_RULES_MIGRATION_ID: &str = "003";
pub const THREADS_MESSAGES_MIGRATION_ID: &str = "004";
pub const REPORT_INDEXES_MIGRATION_ID: &str = "005";
pub const IDEATION_OUTPUTS_MIGRATION_ID: &str = "006";
pub const SHAPING_SCAFFOLDING_MIGRATION_ID: &str = "007";
pub const RESOLUTION_SOURCE_ENRICHMENT_MIGRATION_ID: &str = "008";
pub const DOMAINS_SERVICES_MIGRATION_ID: &str = "009";
pub const SHAPING_TURN_STATE_MIGRATION_ID: &str = "010";
pub const COMMIT_PIN_CONFIDENCE_MIGRATION_ID: &str = "011";
pub const PROPOSAL_LIFECYCLE_MIGRATION_ID: &str = "012";
pub const DISPOSITION_TERMINOLOGY_MIGRATION_ID: &str = "013";
pub const DISPOSITION_EXTERNAL_ACTION_MIGRATION_ID: &str = "014";
pub const DROP_RUNTIME_LEFTOVERS_MIGRATION_ID: &str = "015";
pub const DROP_RULE_CODE_AND_SERVICES_MIGRATION_ID: &str = "016";
pub const REMOVE_SERVICES_SHARDS_MIGRATION_ID: &str = "017";
pub const PROJECTION_STAMP_MIGRATION_ID: &str = "018";
pub const FAMILY_CONTENT_DIGEST_MIGRATION_ID: &str = "019";
pub const UNIT_DIGESTS_MIGRATION_ID: &str = "020";
const INITIAL_SQL: &str = include_str!("../migrations/001_initial_cache.sql");
const SOURCE_REQUIREMENT_SQL: &str =
    include_str!("../migrations/002_sources_requirements_edges.sql");
const RESOLUTIONS_RULES_SQL: &str = include_str!("../migrations/003_resolutions_rules.sql");
const THREADS_MESSAGES_SQL: &str = include_str!("../migrations/004_threads_messages.sql");
const REPORT_INDEXES_SQL: &str = include_str!("../migrations/005_report_indexes.sql");
const IDEATION_OUTPUTS_SQL: &str = include_str!("../migrations/006_ideation_outputs.sql");
const SHAPING_SCAFFOLDING_SQL: &str = include_str!("../migrations/007_shaping_scaffolding.sql");
const RESOLUTION_SOURCE_ENRICHMENT_SQL: &str =
    include_str!("../migrations/008_resolution_source_enrichment.sql");
const DOMAINS_SERVICES_SQL: &str = include_str!("../migrations/009_domains_services.sql");
const SHAPING_TURN_STATE_SQL: &str = include_str!("../migrations/010_shaping_turn_state.sql");
const COMMIT_PIN_CONFIDENCE_SQL: &str = include_str!("../migrations/011_commit_pin_confidence.sql");
const PROPOSAL_LIFECYCLE_SQL: &str = include_str!("../migrations/012_proposal_lifecycle.sql");
const DISPOSITION_TERMINOLOGY_SQL: &str =
    include_str!("../migrations/013_disposition_terminology.sql");
const DISPOSITION_EXTERNAL_ACTION_SQL: &str =
    include_str!("../migrations/014_disposition_external_action.sql");
const DROP_RUNTIME_LEFTOVERS_SQL: &str =
    include_str!("../migrations/015_drop_runtime_leftovers.sql");
const DROP_RULE_CODE_AND_SERVICES_SQL: &str =
    include_str!("../migrations/016_drop_rule_code_and_services.sql");
const REMOVE_SERVICES_SHARDS_SQL: &str =
    include_str!("../migrations/017_remove_services_shards.sql");
const PROJECTION_STAMP_SQL: &str = include_str!("../migrations/018_projection_stamp.sql");
const FAMILY_CONTENT_DIGEST_SQL: &str = include_str!("../migrations/019_family_content_digest.sql");
const UNIT_DIGESTS_SQL: &str = include_str!("../migrations/020_unit_digests.sql");

pub async fn run_migrations(
    pool: &SqlitePool,
    layout: &ProvenanceLayout,
) -> anyhow::Result<Vec<String>> {
    pool.execute("CREATE TABLE IF NOT EXISTS _schema_migrations (id TEXT PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)").await?;
    let mut tx = pool.begin().await?;
    let mut applied = Vec::new();
    for (id, sql) in [
        (INITIAL_MIGRATION_ID, INITIAL_SQL),
        (SOURCE_REQUIREMENT_MIGRATION_ID, SOURCE_REQUIREMENT_SQL),
        (RESOLUTIONS_RULES_MIGRATION_ID, RESOLUTIONS_RULES_SQL),
        (THREADS_MESSAGES_MIGRATION_ID, THREADS_MESSAGES_SQL),
        (REPORT_INDEXES_MIGRATION_ID, REPORT_INDEXES_SQL),
        (IDEATION_OUTPUTS_MIGRATION_ID, IDEATION_OUTPUTS_SQL),
        (SHAPING_SCAFFOLDING_MIGRATION_ID, SHAPING_SCAFFOLDING_SQL),
        (
            RESOLUTION_SOURCE_ENRICHMENT_MIGRATION_ID,
            RESOLUTION_SOURCE_ENRICHMENT_SQL,
        ),
        (DOMAINS_SERVICES_MIGRATION_ID, DOMAINS_SERVICES_SQL),
        (SHAPING_TURN_STATE_MIGRATION_ID, SHAPING_TURN_STATE_SQL),
        (
            COMMIT_PIN_CONFIDENCE_MIGRATION_ID,
            COMMIT_PIN_CONFIDENCE_SQL,
        ),
        (PROPOSAL_LIFECYCLE_MIGRATION_ID, PROPOSAL_LIFECYCLE_SQL),
        (
            DISPOSITION_TERMINOLOGY_MIGRATION_ID,
            DISPOSITION_TERMINOLOGY_SQL,
        ),
        (
            DISPOSITION_EXTERNAL_ACTION_MIGRATION_ID,
            DISPOSITION_EXTERNAL_ACTION_SQL,
        ),
        (
            DROP_RUNTIME_LEFTOVERS_MIGRATION_ID,
            DROP_RUNTIME_LEFTOVERS_SQL,
        ),
        (
            DROP_RULE_CODE_AND_SERVICES_MIGRATION_ID,
            DROP_RULE_CODE_AND_SERVICES_SQL,
        ),
        (
            REMOVE_SERVICES_SHARDS_MIGRATION_ID,
            REMOVE_SERVICES_SHARDS_SQL,
        ),
        (PROJECTION_STAMP_MIGRATION_ID, PROJECTION_STAMP_SQL),
        (
            FAMILY_CONTENT_DIGEST_MIGRATION_ID,
            FAMILY_CONTENT_DIGEST_SQL,
        ),
        (UNIT_DIGESTS_MIGRATION_ID, UNIT_DIGESTS_SQL),
    ] {
        let already_applied: Option<String> =
            sqlx::query_scalar("SELECT id FROM _schema_migrations WHERE id = ?")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if already_applied.is_none() {
            if id == REMOVE_SERVICES_SHARDS_MIGRATION_ID {
                remove_services_shards(layout)?;
            }
            for statement in sql.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                tx.execute(statement).await?;
            }
            sqlx::query("INSERT INTO _schema_migrations (id) VALUES (?)")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            applied.push(id.to_string());
        }
    }
    tx.commit().await?;
    Ok(applied)
}

fn remove_services_shards(layout: &ProvenanceLayout) -> anyhow::Result<()> {
    let scopes_dir = layout.scopes_dir();
    if !scopes_dir.exists() {
        return Ok(());
    }
    for scope in std::fs::read_dir(&scopes_dir)
        .with_context(|| format!("failed to read scopes directory {scopes_dir}"))?
    {
        let scope = scope?;
        if !scope.file_type()?.is_dir() {
            continue;
        }
        let services_dir = scope.path().join("services");
        if !services_dir.exists() {
            continue;
        }
        for shard in std::fs::read_dir(&services_dir).with_context(|| {
            format!(
                "failed to read services directory {}",
                services_dir.display()
            )
        })? {
            let shard = shard?;
            if shard.file_type()?.is_file()
                && shard.path().extension().is_some_and(|ext| ext == "jsonl")
            {
                std::fs::remove_file(shard.path()).with_context(|| {
                    format!(
                        "failed to remove legacy services shard {}",
                        shard.path().display()
                    )
                })?;
            }
        }
    }
    Ok(())
}

pub async fn applied_migrations(pool: &SqlitePool) -> anyhow::Result<Vec<String>> {
    Ok(
        sqlx::query_scalar("SELECT id FROM _schema_migrations ORDER BY id")
            .fetch_all(pool)
            .await?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_layout() -> (tempfile::TempDir, crate::layout::ProvenanceLayout) {
        let directory = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
        (directory, crate::layout::ProvenanceLayout::new(root))
    }

    #[tokio::test]
    async fn migrations_record_initial_cache_schema_once() {
        let (_directory, layout) = test_layout();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        assert_eq!(
            run_migrations(&pool, &layout).await.unwrap(),
            vec![
                "001", "002", "003", "004", "005", "006", "007", "008", "009", "010", "011", "012",
                "013", "014", "015", "016", "017", "018", "019", "020"
            ]
        );
        assert!(run_migrations(&pool, &layout).await.unwrap().is_empty());
        assert_eq!(
            applied_migrations(&pool).await.unwrap(),
            vec![
                "001", "002", "003", "004", "005", "006", "007", "008", "009", "010", "011", "012",
                "013", "014", "015", "016", "017", "018", "019", "020"
            ]
        );
    }

    #[tokio::test]
    async fn lifecycle_migration_creates_assertion_cache() {
        let (_directory, layout) = test_layout();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        run_migrations(&pool, &layout).await.unwrap();
        let table: Option<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'assertion_records'",
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(table.as_deref(), Some("assertion_records"));
        let dispositions: Option<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'dispositions'",
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(dispositions.as_deref(), Some("dispositions"));
    }

    #[tokio::test]
    async fn migration_removes_services_shards_when_present() {
        let (_directory, layout) = test_layout();
        let shard = layout
            .scopes_dir()
            .join("default/services/services-00.jsonl");
        std::fs::create_dir_all(shard.parent().unwrap()).unwrap();
        std::fs::write(&shard, "legacy service\n").unwrap();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        run_migrations(&pool, &layout).await.unwrap();

        assert!(!shard.exists());
    }

    #[tokio::test]
    async fn migration_no_ops_when_services_shards_are_absent() {
        let (_directory, layout) = test_layout();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        run_migrations(&pool, &layout).await.unwrap();

        assert!(!layout.scopes_dir().join("default/services").exists());
    }

    #[tokio::test]
    async fn store_materializes_cleanly_after_services_shard_cleanup() {
        let (_directory, layout) = test_layout();
        std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
        std::fs::write(
            layout.manifest_path(),
            serde_json::to_string(&provenance_core::Manifest::default_with_scope(
                provenance_core::ScopeId::new("default").unwrap(),
                provenance_core::RepoPathPrefix::new("."),
            ))
            .unwrap(),
        )
        .unwrap();
        let shard = layout
            .scopes_dir()
            .join("default/services/services-00.jsonl");
        std::fs::create_dir_all(shard.parent().unwrap()).unwrap();
        std::fs::write(&shard, "not valid json\n").unwrap();

        let report = crate::cache::materialize_state(&layout).await.unwrap();

        assert_eq!(report.records_loaded, 0);
        assert!(!shard.exists());
    }
}
