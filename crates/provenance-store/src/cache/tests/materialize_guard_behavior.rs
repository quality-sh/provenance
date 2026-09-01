use super::super::*;
use super::fixtures::*;
use super::projection_stamp_behavior::seed_integration_shards;
use crate::publication::{events_in_window, read_head_record};
use fs2::FileExt;

fn lock_is_held(layout: &crate::layout::ProvenanceLayout) -> bool {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(layout.publication_lock_path())
        .unwrap();
    if file.try_lock_exclusive().is_ok() {
        let _ = fs2::FileExt::unlock(&file);
        false
    } else {
        true
    }
}

#[tokio::test]
async fn materialization_holds_the_publication_lock_at_commit() {
    let (_dir, layout, _scope) = seeded_layout();
    let fired = std::rc::Rc::new(std::cell::Cell::new(false));
    let probe_layout = layout.clone();
    let probe_fired = fired.clone();
    crate::test_probes::arm("materialize_before_commit", move || {
        probe_fired.set(true);
        anyhow::ensure!(
            lock_is_held(&probe_layout),
            "the publication lock must be held while the transaction commits"
        );
        Ok(())
    });
    materialize_state(&layout).await.unwrap();
    crate::test_probes::disarm("materialize_before_commit");
    assert!(fired.get(), "the commit probe must have run");
}

#[tokio::test]
async fn empty_materialization_holds_the_publication_lock() {
    let (_dir, layout, _scope) = empty_layout();
    let fired = std::rc::Rc::new(std::cell::Cell::new(false));
    let probe_layout = layout.clone();
    let probe_fired = fired.clone();
    crate::test_probes::arm("materialize_empty_under_guard", move || {
        probe_fired.set(true);
        anyhow::ensure!(
            lock_is_held(&probe_layout),
            "empty materialization must run under the publication lock"
        );
        Ok(())
    });
    materialize_empty_state(&layout).await.unwrap();
    crate::test_probes::disarm("materialize_empty_under_guard");
    assert!(fired.get(), "the guard probe must have run");
}

#[tokio::test]
async fn stamp_rows_carry_content_digests_that_rebuild_the_revision_digest() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();

    let stored_digest: String =
        sqlx::query_scalar("SELECT digest FROM projection_revision ORDER BY serial DESC LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let rows: Vec<(String, String, String, i64)> = sqlx::query_as(
        "SELECT scope_id, family, content_digest, record_count FROM projection_family_digests",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 19);
    let rebuilt = revision_digest_from_stored_rows(&rows).unwrap();
    assert_eq!(
        rebuilt, stored_digest,
        "stored content digests must reproduce the revision digest without parsing shards"
    );
}

#[tokio::test]
async fn materialization_advances_the_serial_past_journaled_writes_and_prunes() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    let events_before = events_in_window(&layout, 1, i64::MAX).unwrap();
    assert!(
        !events_before.is_empty(),
        "the seeded writers must have journaled"
    );
    let high_water = events_before
        .iter()
        .map(|event| event.sequence)
        .max()
        .unwrap();

    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let serial: i64 = sqlx::query_scalar("SELECT MAX(serial) FROM projection_revision")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        serial > high_water,
        "the serial covers every journaled write"
    );
    assert!(events_in_window(&layout, 1, serial).unwrap().is_empty());
    assert_eq!(read_head_record(&layout).unwrap(), Some(serial + 1));
}
