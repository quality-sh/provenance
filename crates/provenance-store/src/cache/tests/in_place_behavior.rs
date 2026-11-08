use super::catch_up_behavior::assert_catch_up_equals_rebuild;
use super::fixtures::open;
use super::fixtures::{rewrite_records, seeded_layout};
use crate::cache::{catch_up_state, materialize_state, open_cache, unit_digest, Unit};
use crate::test_probes;
use provenance_macros::verifies;

#[tokio::test]
#[verifies("rule_catch_up_hashes_canonical_state_in_place", examples)]
async fn catch_up_reads_the_state_tree_in_place() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&open(&layout)).await.unwrap();
    rewrite_records(&crate::shards::requirements_path(&layout, &scope), |r| {
        r["statement"] = "Changed".into();
    });
    let repository = open(&layout);
    test_probes::start_recording_reads();
    catch_up_state(&repository).await.unwrap();
    let reads = test_probes::take_recorded_reads();
    assert!(!reads.is_empty());
    for path in reads {
        assert!(
            camino::Utf8Path::new(&path).starts_with(repository.layout().state_dir()),
            "read outside repository state: {path}"
        );
    }
}

#[tokio::test]
#[verifies("rule_catch_up_validates_changed_units_only", examples)]
async fn an_unchanged_pass_parses_no_shard_but_the_manifest() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&open(&layout)).await.unwrap();
    let repository = open(&layout);
    test_probes::start_recording_reads();
    catch_up_state(&repository).await.unwrap();
    assert_eq!(
        test_probes::take_recorded_reads(),
        std::iter::once(repository.layout().manifest_path().to_string()).collect()
    );
}

/// The statement of the one Requirement in the projection.
async fn projected_statement(layout: &crate::layout::ProvenanceLayout) -> String {
    let pool = open_cache(layout).await.unwrap();
    let statement = sqlx::query_scalar("SELECT statement FROM requirements")
        .fetch_one(pool.pool())
        .await
        .unwrap();
    pool.close().await.unwrap();
    statement
}

/// The content digest that the projection stores for the unit of `scope`.
async fn stored_scope_digest(
    layout: &crate::layout::ProvenanceLayout,
    scope: &provenance_core::ScopeId,
) -> String {
    let pool = open_cache(layout).await.unwrap();
    let stored = sqlx::query_scalar("SELECT digest FROM projection_unit_digests WHERE unit = ?")
        .bind(format!("scope:{}", scope.as_str()))
        .fetch_one(pool.pool())
        .await
        .unwrap();
    pool.close().await.unwrap();
    stored
}

#[tokio::test]
#[verifies("rule_catch_up_stores_the_digest_of_parsed_bytes", examples)]
#[verifies("rule_catch_up_reads_units_from_the_view", examples)]
async fn an_edit_after_the_view_is_read_waits_for_the_next_pass() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&open(&layout)).await.unwrap();
    let path = crate::shards::requirements_path(&layout, &scope);
    rewrite_records(&path, |r| r["statement"] = "First edit".into());
    let first = unit_digest(&layout.state_dir(), &Unit::Scope(scope.clone())).unwrap();
    test_probes::arm("catch_up_before_parse", move || {
        rewrite_records(&path, |r| r["statement"] = "Second edit".into());
        Ok(())
    });
    catch_up_state(&open(&layout)).await.unwrap();
    test_probes::disarm("catch_up_before_parse");

    assert_eq!(projected_statement(&layout).await, "First edit");
    assert_eq!(stored_scope_digest(&layout, &scope).await, first);
    let report = catch_up_state(&open(&layout)).await.unwrap();
    assert!(report.revision_committed, "{report:?}");
    assert_eq!(projected_statement(&layout).await, "Second edit");
    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test]
#[verifies("rule_catch_up_stores_the_digest_of_parsed_bytes", examples)]
async fn a_rebuild_stores_the_digest_of_the_bytes_it_loaded() {
    let (_dir, layout, scope) = seeded_layout();
    let loaded = unit_digest(&layout.state_dir(), &Unit::Scope(scope.clone())).unwrap();
    let path = crate::shards::requirements_path(&layout, &scope);
    test_probes::arm("catch_up_before_parse", move || {
        rewrite_records(&path, |r| r["statement"] = "Edit during rebuild".into());
        Ok(())
    });
    materialize_state(&open(&layout)).await.unwrap();
    test_probes::disarm("catch_up_before_parse");

    assert_ne!(projected_statement(&layout).await, "Edit during rebuild");
    assert_eq!(stored_scope_digest(&layout, &scope).await, loaded);
    let report = catch_up_state(&open(&layout)).await.unwrap();
    assert!(report.revision_committed, "{report:?}");
    assert_eq!(projected_statement(&layout).await, "Edit during rebuild");
}

#[cfg(unix)]
#[test]
#[verifies("rule_catch_up_reads_units_from_the_view", examples)]
fn a_symbolic_link_in_a_unit_is_not_canonical_state() {
    let (_dir, layout, scope) = seeded_layout();
    let before = unit_digest(&layout.state_dir(), &Unit::Scope(scope.clone())).unwrap();
    let external = layout.state_dir().join("external.jsonl");
    std::fs::write(&external, "[]").unwrap();
    let link = layout
        .scopes_dir()
        .join(scope.as_str())
        .join("external-link");
    std::os::unix::fs::symlink(&external, &link).unwrap();
    assert_eq!(
        unit_digest(&layout.state_dir(), &Unit::Scope(scope)).unwrap(),
        before
    );
}

#[tokio::test]
#[verifies("rule_catch_up_stores_the_digest_of_parsed_bytes", examples)]
async fn a_manifest_restored_before_hashing_keeps_its_scopes() {
    let (_dir, layout, _scope) = seeded_layout();
    // Open before the test changes the manifest: an open refuses that manifest.
    let repository = open(&layout);
    materialize_state(&repository).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let before = super::catch_up_behavior::dump_family_tables(pool.pool()).await;
    pool.close().await.unwrap();
    let path = layout.manifest_path();
    let original = std::fs::read(&path).unwrap();
    let mut manifest: provenance_core::Manifest = serde_json::from_slice(&original).unwrap();
    manifest.scopes.clear();
    std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    test_probes::arm("catch_up_unit_hashed", move || {
        std::fs::write(&path, &original)?;
        Ok(())
    });
    let result = catch_up_state(&repository).await;
    test_probes::disarm("catch_up_unit_hashed");
    let report = result.unwrap();
    assert!(
        !report.revision_committed,
        "a restored manifest removed scope rows: {report:?}"
    );
    let pool = open_cache(&layout).await.unwrap();
    assert_eq!(
        super::catch_up_behavior::dump_family_tables(pool.pool()).await,
        before
    );
    pool.close().await.unwrap();
}

#[tokio::test]
#[verifies("rule_catch_up_reads_units_from_the_view", examples)]
async fn a_manifest_repaired_after_the_view_is_read_waits_for_the_next_pass() {
    let (_dir, layout, _scope) = seeded_layout();
    // Open before the test changes the manifest: an open refuses that manifest.
    let repository = open(&layout);
    materialize_state(&repository).await.unwrap();
    let path = layout.manifest_path();
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, "{").unwrap();
    test_probes::arm("catch_up_global_before_parse", move || {
        std::fs::write(&path, &original)?;
        Ok(())
    });
    let result = catch_up_state(&repository).await;
    test_probes::disarm("catch_up_global_before_parse");
    assert!(
        result.is_err(),
        "the pass parsed a manifest it did not hash"
    );
    let report = catch_up_state(&repository).await.unwrap();
    assert!(!report.revision_committed, "{report:?}");
}
