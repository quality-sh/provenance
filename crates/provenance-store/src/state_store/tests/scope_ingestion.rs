use super::initialized_store;
use crate::cache::ProjectionFamily;
use crate::layout::ProvenanceLayout;
use crate::publication::with_repository_publication;
use crate::shards;
use crate::state_store::{ScopeShards, StateStore};

fn staged_store() -> (
    tempfile::TempDir,
    StateStore,
    StateStore,
    provenance_core::ScopeId,
    Vec<u8>,
) {
    let (dir, live_store, scope) = initialized_store();
    let manifest_before = std::fs::read(live_store.layout.manifest_path()).unwrap();
    let staged_root = camino::Utf8PathBuf::from_path_buf(dir.path().join("staged-repo")).unwrap();
    let staged_layout = ProvenanceLayout::new(staged_root);
    std::fs::create_dir_all(staged_layout.state_dir()).unwrap();
    std::fs::write(staged_layout.manifest_path(), &manifest_before).unwrap();
    let staged_store = StateStore::new(staged_layout);
    (dir, live_store, staged_store, scope, manifest_before)
}

#[test]
fn scope_import_writes_only_the_imported_shards() {
    let (_dir, live_store, staged_store, scope, manifest_before) = staged_store();

    with_repository_publication(&live_store.layout, || {
        staged_store.import_scope(&scope, &ScopeShards::default())
    })
    .unwrap();

    for family in ProjectionFamily::ALL {
        let path = family.shard_path(&staged_store.layout, &scope);
        if matches!(
            family,
            ProjectionFamily::RequirementReviews | ProjectionFamily::ReviewJournal
        ) {
            assert!(!path.exists(), "{} must stay outside scope import", path);
        } else {
            assert!(std::fs::read(path).unwrap().is_empty());
        }
    }
    assert_eq!(
        std::fs::read(staged_store.layout.manifest_path()).unwrap(),
        manifest_before
    );
    assert!(!shards::ideation_landings_path(&staged_store.layout, &scope).exists());
}

#[test]
fn scope_import_does_not_create_a_staged_repository_lock() {
    let (_dir, live_store, staged_store, scope, _manifest_before) = staged_store();

    with_repository_publication(&live_store.layout, || {
        staged_store.import_scope(&scope, &ScopeShards::default())
    })
    .unwrap();

    assert!(!staged_store.layout.publication_lock_path().exists());
}
