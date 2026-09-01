use super::super::*;
use super::fixtures::*;
use super::projection_stamp_behavior::seed_integration_shards;
use crate::state_store::StateStore;

/// One comparable line per family table: every column of every row, quoted
/// by the database itself, in a settled order.
pub(super) async fn dump_family_tables(pool: &sqlx::SqlitePool) -> Vec<String> {
    let mut dump = Vec::new();
    for family in ProjectionFamily::ALL {
        let name = family.family_name();
        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
                .bind(name)
                .fetch_all(pool)
                .await
                .unwrap();
        let quoted: Vec<String> = columns
            .iter()
            .map(|column| format!("quote({column})"))
            .collect();
        let select = format!(
            "SELECT {} FROM {name} ORDER BY {}",
            quoted.join(" || '|' || "),
            columns.join(", ")
        );
        let rows: Vec<String> = sqlx::query_scalar(&select).fetch_all(pool).await.unwrap();
        dump.push(format!("{name}: {rows:?}"));
    }
    dump
}

async fn stored_digest(pool: &sqlx::SqlitePool) -> String {
    sqlx::query_scalar("SELECT digest FROM projection_revision ORDER BY serial DESC LIMIT 1")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Catch-up output must equal a fresh total rebuild, rows and digest.
async fn assert_catch_up_equals_rebuild(layout: &crate::layout::ProvenanceLayout) {
    let report = catch_up_state(layout).await.unwrap();
    let pool = open_cache(layout).await.unwrap();
    let caught_up_rows = dump_family_tables(&pool).await;
    let caught_up_digest = stored_digest(&pool).await;
    assert_eq!(report.digest, caught_up_digest);
    drop(pool);

    materialize_state(layout).await.unwrap();
    let pool = open_cache(layout).await.unwrap();
    assert_eq!(caught_up_rows, dump_family_tables(&pool).await);
    assert_eq!(caught_up_digest, stored_digest(&pool).await);
}

#[tokio::test]
async fn a_missing_database_routes_to_a_full_rebuild() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    let report = catch_up_state(&layout).await.unwrap();
    assert!(report.rebuilt);
    assert!(report.rows_written > 0);
    let pool = open_cache(&layout).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM requirements")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn a_journaled_write_is_drained_and_matches_a_rebuild() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    materialize_state(&layout).await.unwrap();

    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_after_stamp");

    let report = catch_up_state(&layout).await.unwrap();
    assert!(!report.rebuilt);
    assert!(report.events_drained >= 1, "{report:?}");
    assert!(report.families_rederived >= 1);
    let pool = open_cache(&layout).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sources")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2, "the new source reached the projection");
    drop(pool);
    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test]
async fn a_hand_edited_shard_is_found_by_the_hash_sweep_alone() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    materialize_state(&layout).await.unwrap();

    // Out-of-band edit: append a rule line directly, bypassing every writer
    // and therefore the journal.
    let path = crate::shards::rules_path(&layout, &scope);
    let mut content = std::fs::read_to_string(&path).unwrap();
    let line = format!(
        r#"{{"schema_version":1,"scope_id":"{}","id":"rule_out_of_band","statement":"Edited","status":"active","severity":"low"}}"#,
        scope.as_str()
    );
    content += &line;
    content.push('\n');
    std::fs::write(&path, content).unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    assert!(!report.rebuilt);
    assert_eq!(report.events_drained, 0, "nothing journaled this edit");
    assert!(report.families_rederived >= 1);
    let pool = open_cache(&layout).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rules")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
    drop(pool);
    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test]
async fn an_unchanged_pass_hashes_everything_and_rewrites_nothing() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let digest_before = stored_digest(&pool).await;
    drop(pool);

    let report = catch_up_state(&layout).await.unwrap();
    assert!(!report.rebuilt);
    assert_eq!(report.families_hashed, 19, "every family is hashed, always");
    assert_eq!(report.families_rederived, 0, "no family is reparsed");
    assert_eq!(report.rows_written, 0, "no row is rewritten");
    assert_eq!(
        report.digest, digest_before,
        "unchanged state keeps its digest"
    );
}

#[tokio::test]
async fn draining_the_same_journal_twice_causes_no_row_churn() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    materialize_state(&layout).await.unwrap();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_replayed");

    let first = catch_up_state(&layout).await.unwrap();
    assert!(first.events_drained >= 1);
    let pool = open_cache(&layout).await.unwrap();
    let rows_after_first = dump_family_tables(&pool).await;
    drop(pool);

    // The tail was pruned; a second pass has nothing to drain and nothing
    // to rewrite.
    let second = catch_up_state(&layout).await.unwrap();
    assert_eq!(second.events_drained, 0);
    assert_eq!(second.rows_written, 0);
    let pool = open_cache(&layout).await.unwrap();
    assert_eq!(rows_after_first, dump_family_tables(&pool).await);
    assert_eq!(first.digest, second.digest);
}

#[tokio::test]
async fn a_same_size_edit_with_a_restored_mtime_is_still_caught_without_a_journal() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    materialize_state(&layout).await.unwrap();

    // Same byte length, different content, mtime put back where it was:
    // exactly the edit that metadata comparison cannot see. The journal is
    // deleted, so only the hash can find it.
    let path = crate::shards::rules_path(&layout, &scope);
    let original = std::fs::read_to_string(&path).unwrap();
    let edited = original.replace(
        "Pay overtime after the threshold",
        "Pay overtime befor the threshold",
    );
    assert_eq!(
        original.len(),
        edited.len(),
        "the edit must keep the byte length"
    );
    let mtime = filetime::FileTime::from_last_modification_time(&std::fs::metadata(&path).unwrap());
    std::fs::write(&path, &edited).unwrap();
    filetime::set_file_mtime(&path, mtime).unwrap();
    std::fs::remove_dir_all(layout.cache_dir().join("journal")).unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    assert!(!report.rebuilt);
    assert_eq!(report.events_drained, 0);
    assert_eq!(report.families_hashed, 19);
    assert!(report.families_rederived >= 1, "the hash found the edit");
    let pool = open_cache(&layout).await.unwrap();
    let statement: String =
        sqlx::query_scalar("SELECT statement FROM rules WHERE id = 'rule_schads_pay_001'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(statement.contains("befor"), "{statement}");
}

#[tokio::test]
async fn a_same_size_edit_inside_a_drained_window_without_an_event_is_still_hashed() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    materialize_state(&layout).await.unwrap();

    // A journaled write to another family opens a drain window; the edited
    // family has no event in it. The withdrawn skip rule got exactly this
    // case wrong.
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_window_opener");

    let path = crate::shards::rules_path(&layout, &scope);
    let original = std::fs::read_to_string(&path).unwrap();
    let edited = original.replace(
        "Pay overtime after the threshold",
        "Pay overtime befor the threshold",
    );
    assert_eq!(original.len(), edited.len());
    let mtime = filetime::FileTime::from_last_modification_time(&std::fs::metadata(&path).unwrap());
    std::fs::write(&path, &edited).unwrap();
    filetime::set_file_mtime(&path, mtime).unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    assert!(report.events_drained >= 1, "the window was open");
    assert_eq!(
        report.families_hashed, 19,
        "the drain never excuses the sweep"
    );
    let pool = open_cache(&layout).await.unwrap();
    let statement: String =
        sqlx::query_scalar("SELECT statement FROM rules WHERE id = 'rule_schads_pay_001'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(statement.contains("befor"), "{statement}");
    drop(pool);
    assert_catch_up_equals_rebuild(&layout).await;
}
