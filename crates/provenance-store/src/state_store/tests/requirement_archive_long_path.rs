use super::initialized_store_in;
use super::requirement_archive::{archive, child, requirement};
use provenance_core::RequirementStatus;
use serde_json::json;

/// The archive cascade writes in a staged copy inside the staged copy of the
/// Requirement save. Each copy adds about 76 characters to the path, so a
/// shard there is about 210 characters below the repository root. The test
/// puts the repository root in a directory with a 60-character name below
/// the temporary directory. Then each shard path in the cascade is longer
/// than the 260 characters that a Windows file call accepts without the
/// `\\?\` prefix.
#[test]
fn requirement_archive_under_a_long_repository_path_archives_the_parent_and_child() {
    let directory = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(directory.path().join("r".repeat(60))).unwrap();
    std::fs::create_dir(&root).unwrap();
    let (store, scope) = initialized_store_in(root);
    store
        .create_requirement(
            serde_json::from_value(json!({
                "scope_id":"default", "id":"req_parent", "statement":"The system is safe.",
                "status":"active", "refines":[], "depends_on":[], "supersedes":[]
            }))
            .unwrap(),
        )
        .unwrap();
    child(&store, "req_child", "req_parent");
    archive(&store, "req_parent");
    for id in ["req_parent", "req_child"] {
        assert_eq!(
            requirement(&store, &scope, id).status,
            RequirementStatus::Archived
        );
    }
}
