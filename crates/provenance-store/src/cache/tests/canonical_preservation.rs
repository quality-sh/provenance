use super::fixtures::empty_layout;
use crate::cache::{catch_up_state, materialize_empty_state, materialize_state, open_cache};
use crate::layout::ProvenanceLayout;
use provenance_core::{RepoPathPrefix, Scope, ScopeId};
use std::collections::BTreeMap;

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

fn canonical_bytes(layout: &ProvenanceLayout) -> BTreeMap<String, Vec<u8>> {
    fn collect(root: &std::path::Path, path: &std::path::Path, files: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                collect(root, &path, files);
            } else if path.is_file() {
                files.insert(
                    path.strip_prefix(root).unwrap().to_string_lossy().into_owned(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }

    let mut files = BTreeMap::new();
    collect(
        layout.state_dir().as_std_path(),
        layout.state_dir().as_std_path(),
        &mut files,
    );
    files
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
    sqlx::query("UPDATE _cache_metadata SET schema_digest = 'incompatible'")
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

#[tokio::test]
async fn incompatible_cache_rebuild_keeps_canonical_state_byte_identical() {
    let (_directory, layout, _scope) = empty_layout();
    materialize_state(&layout).await.unwrap();
    let before = canonical_bytes(&layout);
    let cache = open_cache(&layout).await.unwrap();
    sqlx::query("UPDATE _cache_metadata SET schema_digest = 'incompatible'")
        .execute(cache.pool())
        .await
        .unwrap();
    cache.close().await.unwrap();

    let report = catch_up_state(&layout).await.unwrap();

    assert!(report.cache_recreated);
    assert_eq!(canonical_bytes(&layout), before);
}
