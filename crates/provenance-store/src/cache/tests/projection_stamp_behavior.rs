use super::super::*;
use super::fixtures::*;
use sqlx::{Executor, Row};

async fn revision_row(layout: &ProvenanceLayout) -> anyhow::Result<Option<(i64, String, String)>> {
    let pool = open_cache(layout).await?;
    let row = pool
        .fetch_optional("SELECT serial, instance_id, digest FROM projection_revision")
        .await?;
    Ok(row.map(|row| {
        (
            row.get::<i64, _>("serial"),
            row.get::<String, _>("instance_id"),
            row.get::<String, _>("digest"),
        )
    }))
}

#[tokio::test]
async fn materialize_stamps_a_projection_revision() {
    let (_dir, layout, _scope) = seeded_layout();

    materialize_state(&layout).await.unwrap();

    let (serial, instance_id, digest) = revision_row(&layout).await.unwrap().unwrap();
    assert!(serial >= 1, "serial must be a positive revision number");
    assert!(
        !instance_id.is_empty(),
        "every projection names its instance"
    );
    let expected = projection_digest(&layout).unwrap();
    assert_eq!(
        digest, expected,
        "stamp digest must cover every stored family"
    );
}

#[tokio::test]
async fn rematerializing_the_same_database_keeps_the_projection_instance() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let (_, instance_before, _) = revision_row(&layout).await.unwrap().unwrap();

    materialize_state(&layout).await.unwrap();

    let (_, instance_after, _) = revision_row(&layout).await.unwrap().unwrap();
    assert_eq!(instance_before, instance_after);
}

#[tokio::test]
async fn rebuilding_after_total_cache_loss_mints_a_new_projection_instance() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let (_, instance_before, _) = revision_row(&layout).await.unwrap().unwrap();

    std::fs::remove_file(layout.cache_db_path()).unwrap();
    materialize_state(&layout).await.unwrap();

    let (_, instance_after, _) = revision_row(&layout).await.unwrap().unwrap();
    assert_ne!(instance_before, instance_after);
}

#[tokio::test]
async fn empty_materialization_writes_no_revision() {
    let (_dir, layout, _scope) = empty_layout();

    materialize_empty_state(&layout).await.unwrap();

    assert!(revision_row(&layout).await.unwrap().is_none());
}

#[tokio::test]
async fn projection_migration_creates_family_and_stamp_tables() {
    let (_dir, layout, _scope) = empty_layout();
    let pool = open_cache(&layout).await.unwrap();

    crate::migrations::run_migrations(&pool, &layout)
        .await
        .unwrap();

    for table in [
        "implementation_bindings",
        "verification_bindings",
        "requirement_reviews",
        "projection_revision",
        "projection_family_digests",
    ] {
        let found: Option<String> =
            sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?")
                .bind(table)
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert_eq!(found.as_deref(), Some(table), "missing table {table}");
    }
}

#[tokio::test]
async fn materialized_binding_families_answer_from_rows() {
    let (_dir, layout, scope) = seeded_layout();
    let bindings = crate::shards::implementation_bindings_path(&layout, &scope);
    std::fs::create_dir_all(bindings.parent().unwrap()).unwrap();
    std::fs::write(
        &bindings,
        r#"{"schema_version":1,"scope_id":"default","id":"impl_test_pay","rule_id":"rule_schads_pay_001","declared_by":"test","file":"src/lib.rs","symbol":"pay"}
"#,
    )
    .unwrap();

    materialize_state(&layout).await.unwrap();

    let pool = open_cache(&layout).await.unwrap();
    let count: i64 = pool
        .fetch_one("SELECT COUNT(*) AS n FROM implementation_bindings")
        .await
        .unwrap()
        .get("n");
    assert_eq!(count, 1, "implementation bindings must be projected");
    let reviews: i64 = pool
        .fetch_one("SELECT COUNT(*) AS n FROM requirement_reviews")
        .await
        .unwrap()
        .get("n");
    assert_eq!(reviews, 0);
}
