use super::catch_up_behavior::assert_catch_up_equals_rebuild;
use super::fixtures::{rewrite_records, seeded_layout};
use crate::cache::{catch_up_state, materialize_state, open_cache, unit_digest, Unit};
use crate::test_probes;
use provenance_macros::verifies;

#[tokio::test]
#[verifies("rule_catch_up_hashes_canonical_state_in_place", examples)]
async fn catch_up_reads_the_state_tree_in_place() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    rewrite_records(&crate::shards::requirements_path(&layout, &scope), |r| {
        r["statement"] = "Changed".into();
    });
    test_probes::start_recording_reads();
    catch_up_state(&layout).await.unwrap();
    let reads = test_probes::take_recorded_reads();
    assert!(!reads.is_empty());
    for path in reads {
        assert!(
            camino::Utf8Path::new(&path).starts_with(layout.state_dir()),
            "read outside repository state: {path}"
        );
    }
}

#[tokio::test]
#[verifies("rule_catch_up_validates_changed_units_only", examples)]
async fn an_unchanged_pass_parses_no_shard_but_the_manifest() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    test_probes::start_recording_reads();
    catch_up_state(&layout).await.unwrap();
    assert_eq!(
        test_probes::take_recorded_reads(),
        std::iter::once(layout.manifest_path().to_string()).collect()
    );
}

#[tokio::test]
#[verifies("rule_catch_up_stores_the_digest_of_parsed_bytes", examples)]
async fn an_edit_between_hash_and_parse_is_hashed_again() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let path = crate::shards::requirements_path(&layout, &scope);
    rewrite_records(&path, |r| r["statement"] = "First edit".into());
    test_probes::arm("catch_up_before_parse", move || {
        rewrite_records(&path, |r| r["statement"] = "Second edit".into());
        Ok(())
    });
    catch_up_state(&layout).await.unwrap();
    test_probes::disarm("catch_up_before_parse");
    let pool = open_cache(&layout).await.unwrap();
    let statement: String = sqlx::query_scalar("SELECT statement FROM requirements")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(statement, "Second edit");
    let stored: String =
        sqlx::query_scalar("SELECT digest FROM projection_unit_digests WHERE unit = ?")
            .bind(format!("scope:{}", scope.as_str()))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        stored,
        unit_digest(&layout.state_dir(), &Unit::Scope(scope)).unwrap()
    );
    pool.close().await;
    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test]
#[verifies("rule_catch_up_stores_the_digest_of_parsed_bytes", examples)]
async fn a_rebuild_stores_the_digest_of_the_bytes_it_loaded() {
    let (_dir, layout, scope) = seeded_layout();
    let path = crate::shards::requirements_path(&layout, &scope);
    test_probes::arm("catch_up_before_parse", move || {
        rewrite_records(&path, |r| r["statement"] = "Edit during rebuild".into());
        Ok(())
    });
    materialize_state(&layout).await.unwrap();
    test_probes::disarm("catch_up_before_parse");
    let pool = open_cache(&layout).await.unwrap();
    let statement: String = sqlx::query_scalar("SELECT statement FROM requirements")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(statement, "Edit during rebuild");
    let stored: String =
        sqlx::query_scalar("SELECT digest FROM projection_unit_digests WHERE unit = ?")
            .bind(format!("scope:{}", scope.as_str()))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        stored,
        unit_digest(&layout.state_dir(), &Unit::Scope(scope)).unwrap()
    );
    pool.close().await;
    assert!(!catch_up_state(&layout).await.unwrap().revision_committed);
}

#[cfg(unix)]
#[test]
fn a_unit_with_a_symbolic_link_is_refused_before_parsing() {
    let (_dir, layout, scope) = seeded_layout();
    let external = layout.state_dir().join("external.jsonl");
    std::fs::write(&external, "[]").unwrap();
    let link = layout
        .scopes_dir()
        .join(scope.as_str())
        .join("external-link");
    std::os::unix::fs::symlink(&external, &link).unwrap();
    let result = unit_digest(&layout.state_dir(), &Unit::Scope(scope));
    assert!(result.is_err(), "an unhashed link was accepted: {result:?}");
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("unsupported state entry"));
}

#[tokio::test]
#[verifies("rule_catch_up_stores_the_digest_of_parsed_bytes", examples)]
async fn a_manifest_restored_before_hashing_keeps_its_scopes() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let before = super::catch_up_behavior::dump_family_tables(&pool).await;
    pool.close().await;
    let path = layout.manifest_path();
    let original = std::fs::read(&path).unwrap();
    let mut manifest: provenance_core::Manifest = serde_json::from_slice(&original).unwrap();
    manifest.scopes.clear();
    std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    test_probes::arm("catch_up_unit_hashed", move || {
        std::fs::write(&path, &original)?;
        Ok(())
    });
    let result = catch_up_state(&layout).await;
    test_probes::disarm("catch_up_unit_hashed");
    let report = result.unwrap();
    assert!(
        !report.revision_committed,
        "a restored manifest removed scope rows: {report:?}"
    );
    let pool = open_cache(&layout).await.unwrap();
    assert_eq!(
        super::catch_up_behavior::dump_family_tables(&pool).await,
        before
    );
    pool.close().await;
}

#[tokio::test]
#[verifies("rule_catch_up_stores_the_digest_of_parsed_bytes", examples)]
async fn a_manifest_repaired_after_hashing_is_retried() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let path = layout.manifest_path();
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, "{").unwrap();
    test_probes::arm("catch_up_global_before_parse", move || {
        std::fs::write(&path, &original)?;
        Ok(())
    });
    let result = catch_up_state(&layout).await;
    test_probes::disarm("catch_up_global_before_parse");
    assert!(
        result.is_ok(),
        "the repaired manifest was not retried: {result:?}"
    );
    assert!(!result.unwrap().revision_committed);
}
