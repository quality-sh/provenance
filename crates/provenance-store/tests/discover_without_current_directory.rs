//! Discovery from an explicit absolute root when the current directory is
//! gone.
//!
//! The test changes the current directory of the process and then removes
//! that directory. It is the only test in this binary, so no other test runs
//! in the same process.

use provenance_core::{Manifest, RepoPathPrefix, ScopeId};
use provenance_macros::verifies;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::repository::Repository;

#[test]
#[verifies("rule_sdk_project_discovery", examples)]
fn discover_resolves_an_explicit_root_without_reading_the_current_directory() {
    let dir = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(dir.path().to_str().unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let manifest =
        Manifest::default_with_scope(ScopeId::new("a").unwrap(), RepoPathPrefix::new("a"));
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let root =
        camino::Utf8PathBuf::from_path_buf(dunce::canonicalize(dir.path()).unwrap()).unwrap();

    // Windows does not let a process remove its current directory, so only
    // Unix makes the current directory unavailable.
    #[cfg(unix)]
    {
        let gone = tempfile::tempdir().unwrap();
        std::env::set_current_dir(gone.path()).unwrap();
        std::fs::remove_dir(gone.path()).unwrap();
        assert!(std::env::current_dir().is_err());
    }

    let repository = Repository::discover(Some(root.clone()), "a").unwrap();
    assert_eq!(repository.scope().as_str(), "a");
    assert_eq!(repository.working_tree(), root);
}
