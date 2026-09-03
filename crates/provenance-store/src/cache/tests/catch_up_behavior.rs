use super::super::*;
use super::fixtures::*;
use super::projection_stamp_behavior::seed_integration_shards;

/// One line per family table: every column of every row, quoted by the
/// database, in a settled order.
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

/// Catch-up output must equal a fresh rebuild, rows and digest.
pub(super) async fn assert_catch_up_equals_rebuild(layout: &crate::layout::ProvenanceLayout) {
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
async fn a_hand_edited_shard_is_found_by_the_hash_sweep_alone() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    materialize_state(&layout).await.unwrap();

    // Append a rule line directly, bypassing every writer.
    let path = crate::shards::rules_path(&layout, &scope);
    let mut content = std::fs::read_to_string(&path).unwrap();
    let line = format!(
        r#"{{"schema_version":1,"scope_id":"{}","id":"rule_out_of_band","statement":"Edited","status":"active","severity":"low","requirement_ids":["req_schads_overtime"]}}"#,
        scope.as_str()
    );
    content += &line;
    content.push('\n');
    std::fs::write(&path, content).unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    assert!(!report.rebuilt);
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

    let hashed = std::rc::Rc::new(std::cell::Cell::new(0u64));
    let counter = hashed.clone();
    crate::test_probes::arm("catch_up_unit_hashed", move || {
        counter.set(counter.get() + 1);
        Ok(())
    });
    let report = catch_up_state(&layout).await.unwrap();
    crate::test_probes::disarm("catch_up_unit_hashed");
    assert!(!report.rebuilt);
    // The probe counts real hash calls. The report must agree.
    assert_eq!(hashed.get(), 2, "every unit is hashed, always");
    assert_eq!(report.units_hashed, hashed.get());
    assert_eq!(report.families_rederived, 0, "no family is reparsed");
    assert_eq!(report.rows_written, 0, "no row is rewritten");
    assert_eq!(
        report.digest, digest_before,
        "unchanged state keeps its digest"
    );
}

#[tokio::test]
async fn a_same_size_edit_with_a_restored_mtime_is_still_caught() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    materialize_state(&layout).await.unwrap();

    // Same byte length, different content, mtime restored. Metadata
    // comparison cannot see this edit.
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

    let report = catch_up_state(&layout).await.unwrap();
    assert!(!report.rebuilt);
    assert_eq!(report.units_hashed, 2, "one scope unit and the global unit");
    assert!(report.families_rederived >= 1, "the hash found the edit");
    let pool = open_cache(&layout).await.unwrap();
    let statement: String =
        sqlx::query_scalar("SELECT statement FROM rules WHERE id = 'rule_schads_pay_001'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(statement.contains("befor"), "{statement}");
}
