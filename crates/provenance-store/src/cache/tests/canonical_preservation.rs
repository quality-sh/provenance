use super::fixtures::empty_layout;
use crate::cache::{catch_up_state, materialize_empty_state, materialize_state, open_cache};
use crate::layout::ProvenanceLayout;
use crate::migrations::REMOVE_SERVICES_SHARDS_MIGRATION_ID;
use provenance_core::{RepoPathPrefix, Scope, ScopeId};

const LEGACY_BYTES: &[u8] = b"{\"legacy\":\"service\"}\n";
const SENTINEL_BYTES: &[u8] = b"external sentinel\n";

struct CanonicalCanaries {
    legacy_shard: camino::Utf8PathBuf,
    sentinel: std::path::PathBuf,
    _external: tempfile::TempDir,
}

impl CanonicalCanaries {
    fn assert_unchanged(&self) {
        let legacy = std::fs::read(&self.legacy_shard).ok();
        let sentinel = std::fs::read(&self.sentinel).ok();
        assert_eq!(
            (legacy.as_deref(), sentinel.as_deref()),
            (Some(LEGACY_BYTES), Some(SENTINEL_BYTES))
        );
    }
}

fn seed_canaries(layout: &ProvenanceLayout) -> CanonicalCanaries {
    let legacy_shard = layout
        .scopes_dir()
        .join("default/services/services-00.jsonl");
    std::fs::create_dir_all(legacy_shard.parent().unwrap()).unwrap();
    std::fs::write(&legacy_shard, LEGACY_BYTES).unwrap();

    let external = tempfile::tempdir().unwrap();
    let sentinel = external.path().join("sentinel.jsonl");
    std::fs::write(&sentinel, SENTINEL_BYTES).unwrap();
    let linked_scope = layout.scopes_dir().join("linked");
    std::fs::create_dir_all(&linked_scope).unwrap();
    std::os::unix::fs::symlink(external.path(), linked_scope.join("services")).unwrap();

    CanonicalCanaries {
        legacy_shard,
        sentinel,
        _external: external,
    }
}

fn add_linked_scope(layout: &ProvenanceLayout) {
    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.scopes.push(Scope {
        id: ScopeId::new("linked").unwrap(),
        path_prefix: RepoPathPrefix::new("linked"),
    });
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

#[tokio::test]
async fn fresh_cache_initialization_preserves_canonical_services_files() {
    let (_directory, layout, _scope) = empty_layout();
    let canaries = seed_canaries(&layout);

    materialize_empty_state(&layout).await.unwrap();

    canaries.assert_unchanged();
}

#[tokio::test]
async fn full_rebuild_preserves_canonical_services_files_before_a_later_failure() {
    let (_directory, layout, _scope) = empty_layout();
    add_linked_scope(&layout);
    let canaries = seed_canaries(&layout);

    let error = materialize_state(&layout).await.unwrap_err();

    canaries.assert_unchanged();
    assert!(
        error.to_string().contains("unsupported state entry"),
        "{error}"
    );
}

#[tokio::test]
async fn existing_cache_catch_up_preserves_canonical_services_files_before_a_later_failure() {
    let (_directory, layout, _scope) = empty_layout();
    materialize_state(&layout).await.unwrap();
    let cache = open_cache(&layout).await.unwrap();
    sqlx::query("DELETE FROM _schema_migrations WHERE id = ?")
        .bind(REMOVE_SERVICES_SHARDS_MIGRATION_ID)
        .execute(cache.pool())
        .await
        .unwrap();
    cache.close().await.unwrap();
    add_linked_scope(&layout);
    let canaries = seed_canaries(&layout);

    let error = catch_up_state(&layout).await.unwrap_err();

    canaries.assert_unchanged();
    assert!(
        error.to_string().contains("unsupported state entry"),
        "{error}"
    );
}
