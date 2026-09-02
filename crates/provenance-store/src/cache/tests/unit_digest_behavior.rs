//! The hash units: what they cover, what they ignore, and what they own.

use super::super::*;
use super::catch_up_behavior::assert_catch_up_equals_rebuild;
use super::fixtures::*;
use super::projection_stamp_behavior::seed_integration_shards;
use provenance_core::ScopeId;

fn scope_digest(layout: &crate::layout::ProvenanceLayout, scope: &ScopeId) -> String {
    unit_digest(&layout.state_dir(), &Unit::Scope(scope.clone())).unwrap()
}

#[test]
fn swapping_two_shards_that_share_a_basename_moves_the_scope_digest() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    let before = scope_digest(&layout, &scope);

    let implementations = crate::shards::implementation_bindings_path(&layout, &scope);
    let verifications = crate::shards::verification_bindings_path(&layout, &scope);
    let left = std::fs::read(&implementations).unwrap();
    let right = std::fs::read(&verifications).unwrap();
    assert_ne!(left, right);
    std::fs::write(&implementations, &right).unwrap();
    std::fs::write(&verifications, &left).unwrap();

    assert_ne!(
        scope_digest(&layout, &scope),
        before,
        "the digest frames the relative path, not the basename"
    );
}

#[test]
fn temporary_write_residue_does_not_move_a_unit_digest() {
    let (_dir, layout, scope) = seeded_layout();
    let before = scope_digest(&layout, &scope);
    let global_before = unit_digest(&layout.state_dir(), &Unit::Global).unwrap();

    let requirements_dir = crate::shards::requirements_path(&layout, &scope)
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::write(requirements_dir.join(".tmpAbC123"), b"half-written").unwrap();
    std::fs::write(layout.edges_dir().join(".tmpXyZ789"), b"half-written").unwrap();

    assert_eq!(scope_digest(&layout, &scope), before);
    assert_eq!(
        unit_digest(&layout.state_dir(), &Unit::Global).unwrap(),
        global_before
    );
}

#[test]
fn the_global_unit_covers_every_canonical_file_outside_scopes() {
    let (_dir, layout, scope) = seeded_layout();
    let before = unit_digest(&layout.state_dir(), &Unit::Global).unwrap();
    let scope_before = scope_digest(&layout, &scope);

    std::fs::write(layout.state_dir().join("dictionary.json"), b"{}").unwrap();
    let after_dictionary = unit_digest(&layout.state_dir(), &Unit::Global).unwrap();
    assert_ne!(after_dictionary, before, "the dictionary is global");

    let mut manifest = std::fs::read_to_string(layout.manifest_path()).unwrap();
    manifest.push('\n');
    std::fs::write(layout.manifest_path(), manifest).unwrap();
    assert_ne!(
        unit_digest(&layout.state_dir(), &Unit::Global).unwrap(),
        after_dictionary,
        "the manifest is global"
    );
    assert_eq!(scope_digest(&layout, &scope), scope_before, "scopes stand");
}

#[tokio::test]
async fn a_scope_departure_never_deletes_edge_rows() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let edges_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM edges")
        .fetch_one(&pool)
        .await
        .unwrap();
    pool.close().await;
    assert!(edges_before > 0);

    // The scope leaves the manifest; the global edge shard keeps its rows,
    // and rebuild loads the shard regardless of the manifest.
    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.scopes.clear();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    assert!(!report.rebuilt);
    let pool = open_cache(&layout).await.unwrap();
    let edges_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM edges")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        edges_after, edges_before,
        "edge rows belong to the global unit"
    );
    pool.close().await;
    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test]
async fn a_changed_scope_rewrites_only_the_families_whose_content_moved() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    materialize_state(&layout).await.unwrap();

    let rules = crate::shards::rules_path(&layout, &scope);
    let edited = std::fs::read_to_string(&rules)
        .unwrap()
        .replace("Pay overtime", "Pay double overtime");
    std::fs::write(&rules, edited).unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    assert_eq!(report.units_hashed, 2, "one scope unit and the global unit");
    assert_eq!(report.families_rederived, 1, "only rules moved: {report:?}");
    assert_eq!(report.rows_written, 1);
    assert_catch_up_equals_rebuild(&layout).await;
}

#[test]
fn moving_a_shard_between_directories_moves_the_scope_digest() {
    // One file, same bytes, same basename, different directory: a digest
    // that framed only the basename would not see the move, but readers
    // would derive a different family from it. The scope holds nothing
    // else, so the moved file's position in the stream cannot give it away.
    let (_dir, layout, scope) = empty_layout();
    let implementations = crate::shards::implementation_bindings_path(&layout, &scope);
    let verifications = crate::shards::verification_bindings_path(&layout, &scope);
    std::fs::create_dir_all(implementations.parent().unwrap()).unwrap();
    std::fs::write(&implementations, b"{\"moved\":true}\n").unwrap();
    let before = scope_digest(&layout, &scope);

    std::fs::remove_file(&implementations).unwrap();
    std::fs::create_dir_all(verifications.parent().unwrap()).unwrap();
    std::fs::write(&verifications, b"{\"moved\":true}\n").unwrap();

    assert_ne!(scope_digest(&layout, &scope), before);
}

#[tokio::test]
async fn a_scope_change_never_touches_edge_rows() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let edges_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM edges")
        .fetch_one(&pool)
        .await
        .unwrap();
    pool.close().await;

    // Only the scope unit moves; the global unit, and so the edges table,
    // must stand untouched through the pass.
    let rules = crate::shards::rules_path(&layout, &scope);
    let edited = std::fs::read_to_string(&rules)
        .unwrap()
        .replace("Pay overtime", "Pay triple overtime");
    std::fs::write(&rules, edited).unwrap();
    let report = catch_up_state(&layout).await.unwrap();
    assert_eq!(report.families_rederived, 1, "{report:?}");

    let pool = open_cache(&layout).await.unwrap();
    let edges_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM edges")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(edges_after, edges_before);
    pool.close().await;
    assert_catch_up_equals_rebuild(&layout).await;
}
