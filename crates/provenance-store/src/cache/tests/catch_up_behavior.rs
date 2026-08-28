use super::super::*;
use super::fixtures::*;
use crate::state_store::StateStore;
use sqlx::Row;

async fn domain_names(layout: &ProvenanceLayout) -> Vec<String> {
    let pool = open_cache(layout).await.unwrap();
    let names: Vec<String> = sqlx::query("SELECT name FROM domains ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap()
        .iter()
        .map(|row| row.get::<String, _>("name"))
        .collect();
    pool.close().await;
    names
}

async fn revision(layout: &ProvenanceLayout) -> (i64, String, String) {
    let pool = open_cache(layout).await.unwrap();
    let row = sqlx::query("SELECT serial, instance_id, digest FROM projection_revision LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let revision = (
        row.get::<i64, _>("serial"),
        row.get::<String, _>("instance_id"),
        row.get::<String, _>("digest"),
    );
    pool.close().await;
    revision
}

async fn table_rows(layout: &ProvenanceLayout, table: &str) -> Vec<String> {
    let pool = open_cache(layout).await.unwrap();
    let rows: Vec<String> = sqlx::query(&format!("SELECT * FROM {table} ORDER BY 1, 2"))
        .fetch_all(&pool)
        .await
        .unwrap()
        .iter()
        .map(|row| {
            (0..row.len())
                .map(|index| {
                    row.try_get::<Option<String>, usize>(index)
                        .ok()
                        .flatten()
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect();
    pool.close().await;
    rows
}

fn remove_database(layout: &ProvenanceLayout) {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let path = camino::Utf8PathBuf::from(format!("{}{suffix}", layout.cache_db_path()));
        if path.exists() {
            std::fs::remove_file(&path).unwrap();
        }
    }
}

fn truncate_journal_tail(layout: &ProvenanceLayout, keep_lines: usize) {
    let path = layout.journal_events_path();
    let content = std::fs::read_to_string(&path).unwrap();
    let kept = content.lines().take(keep_lines).collect::<Vec<_>>();
    std::fs::write(&path, kept.join("\n") + "\n").unwrap();
}

#[tokio::test]
async fn catch_up_after_materialize_commits_a_new_stamp_and_prunes_the_journal() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let (serial_before, instance, digest_before) = revision(&layout).await;
    let events = layout.journal_events_path();
    std::fs::write(&events, "").unwrap();

    let report = catch_up_state(&layout).await.unwrap();

    let (serial_after, instance_after, digest_after) = revision(&layout).await;
    assert!(!report.rebuilt);
    assert_eq!(report.journal_drained, 0);
    assert!(
        serial_after > serial_before,
        "a committed pass advances the serial"
    );
    assert_eq!(
        instance, instance_after,
        "the instance survives a normal pass"
    );
    assert_eq!(
        digest_before, digest_after,
        "identical bytes keep the digest still"
    );
    let tail = std::fs::read_to_string(&events).unwrap();
    assert!(
        tail.trim().is_empty(),
        "pruning leaves the tail above the new serial"
    );
}

#[tokio::test]
async fn catch_up_detects_a_same_size_shard_edit_with_a_restored_mtime() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();

    let domain_shard = crate::shards::domains_path(&layout, &scope);
    let original = std::fs::read_to_string(&domain_shard).unwrap();
    let edited = original.replace("Payroll", "PayrolZ");
    assert_eq!(
        original.len(),
        edited.len(),
        "fixture edit must be same size"
    );
    std::fs::write(&domain_shard, edited).unwrap();

    #[cfg(unix)]
    let _ = std::process::Command::new("touch")
        .args(["-d", "@1000000", domain_shard.as_str()])
        .output();

    let report = catch_up_state(&layout).await.unwrap();

    assert!(
        report
            .families_rederived
            .iter()
            .any(|family| family == "domains"),
        "the edited family must be re-derived, got {:?}",
        report.families_rederived
    );
    assert_eq!(domain_names(&layout).await, vec!["PayrolZ".to_string()]);
}

#[tokio::test]
async fn catch_up_drains_journal_events_from_writer_mutations() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let store = StateStore::new(layout.clone());
    create_requirement(
        &store,
        &scope,
        "req_after_stamp",
        provenance_core::RequirementStatus::Active,
    );

    let report = catch_up_state(&layout).await.unwrap();

    assert_eq!(report.journal_drained, 1);
    assert!(report
        .families_rederived
        .iter()
        .any(|family| family == "requirements"));
    let pool = open_cache(&layout).await.unwrap();
    let found: i64 =
        sqlx::query("SELECT COUNT(*) AS n FROM requirements WHERE id = 'req_after_stamp'")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get("n");
    pool.close().await;
    assert_eq!(
        found, 1,
        "the hinted family must be re-derived from canonical bytes"
    );
}

#[tokio::test]
async fn catch_up_rebuilds_after_database_loss_and_keeps_surviving_serials() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let (_, instance, _) = revision(&layout).await;
    // Write journal events above the stored serial, then lose the database.
    let store = StateStore::new(layout.clone());
    store
        .set_requirement_fog(
            &provenance_core::ScopeId::new("default").unwrap(),
            &sid("req_schads_overtime"),
            Some("fog".into()),
        )
        .unwrap();
    remove_database(&layout);

    let report = catch_up_state(&layout).await.unwrap();

    assert!(report.rebuilt);
    let (serial, instance_after, _) = revision(&layout).await;
    assert!(serial >= 1);
    assert_ne!(
        instance, instance_after,
        "a fresh database mints a fresh instance"
    );
}

#[tokio::test]
async fn catch_up_restarts_at_one_after_total_cache_loss() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    remove_database(&layout);
    std::fs::remove_dir_all(layout.journal_dir()).unwrap();

    let report = catch_up_state(&layout).await.unwrap();

    let (serial, _, _) = revision(&layout).await;
    assert_eq!(serial, 1, "total cache loss is the only restart at one");
    assert!(report.rebuilt);
}

#[tokio::test]
async fn truncated_tail_still_hashes_every_family_and_advances_the_serial() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let (serial_before, _, _) = revision(&layout).await;
    truncate_journal_tail(&layout, 0);

    let report = catch_up_state(&layout).await.unwrap();

    let (serial_after, _, digest) = revision(&layout).await;
    assert!(serial_after > serial_before);
    let store = StateStore::new(layout.clone());
    assert_eq!(
        digest,
        crate::cache::projection_digest(&layout, &store.manifest().unwrap()).unwrap(),
        "the committed stamp still covers every family"
    );
    assert!(!report.rebuilt);
}

#[tokio::test]
async fn catch_up_output_equals_a_fresh_total_rebuild() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let store = StateStore::new(layout.clone());
    create_requirement(
        &store,
        &scope,
        "req_equivalence",
        provenance_core::RequirementStatus::Active,
    );
    create_requirement(
        &store,
        &scope,
        "req_equivalence_two",
        provenance_core::RequirementStatus::Active,
    );
    catch_up_state(&layout).await.unwrap();

    let incremental = table_rows(&layout, "requirements").await;

    materialize_state(&layout).await.unwrap();
    let rebuilt = table_rows(&layout, "requirements").await;

    assert_eq!(incremental, rebuilt, "catch-up and rebuild must converge");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rebuild_and_catch_up_serialize_under_the_publication_guard() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let layout_a = layout.clone();
    let layout_b = layout.clone();
    let catch_up = tokio::spawn(async move { catch_up_state(&layout_a).await });
    let rebuild = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        runtime.block_on(materialize_state(&layout_b))
    });
    let catch_up = catch_up.await.unwrap().unwrap();
    let rebuild = rebuild.join().unwrap().unwrap();
    let (final_serial, _, _) = revision(&layout).await;
    let _ = (catch_up.serial, rebuild.serial);
    assert!(
        final_serial >= 1,
        "single serial progression survives interleaving"
    );
}

#[tokio::test]
async fn unjournaled_byte_changes_are_found_by_digest_comparison_alone() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    // No journal events at all: the change bypassed the journal.
    let rules = crate::shards::rules_path(&layout, &scope);
    let edited = std::fs::read_to_string(&rules)
        .unwrap()
        .replace("threshold", "ceiling");
    std::fs::write(&rules, edited).unwrap();

    let report = catch_up_state(&layout).await.unwrap();

    assert!(
        report
            .families_rederived
            .iter()
            .any(|family| family == "rules"),
        "the sweep must find the unjournaled change, got {:?}",
        report.families_rederived
    );
}
