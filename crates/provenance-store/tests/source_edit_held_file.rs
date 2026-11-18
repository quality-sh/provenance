//! A source edit while a reader has a state file open, as an editor, a
//! virus scanner or a second command can have it. On Linux and macOS a
//! rename always replaces an open file, so this test can fail only on
//! Windows.

use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{Manifest, RepoPathPrefix, ScopeId};
use provenance_macros::verifies;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::operations::files::RepositoryFiles;
use provenance_store::publication::with_staged_state_and_source_edit;
use provenance_store::repository::Repository;

/// The reader keeps the manifest open with the sharing of `std::fs`, which
/// lets other handles delete the file, while a source edit replaces a
/// working-tree file and adds a Requirement.
#[test]
#[verifies("rule_write_passes_while_a_record_file_is_held", examples)]
fn a_source_edit_passes_while_a_reader_has_a_state_file_open() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(dunce::canonicalize(directory.path()).unwrap()).unwrap();
    let layout = ProvenanceLayout::new(root.clone());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let manifest =
        Manifest::default_with_scope(ScopeId::new("default").unwrap(), RepoPathPrefix::new("."));
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(root.join("source.txt"), b"before\n").unwrap();
    let repository = Repository::open(&root, "default").unwrap();
    let held = RepositoryFiles::open(&root)
        .unwrap()
        .read_bounded(Utf8Path::new("source.txt"), 1024)
        .unwrap();
    let requirement = serde_json::from_value(serde_json::json!({
        "scope_id": "default", "id": "req_a",
        "statement": "The system stores records.",
        "status": "discovery", "depends_on": [], "supersedes": [],
        "description": null, "refines": [],
        "spawned_by": null, "origin_thread": null, "origin_message": null
    }))
    .unwrap();
    let reader = std::fs::File::open(layout.manifest_path()).unwrap();

    let written = repository.store().with_repository_publication(|section| {
        with_staged_state_and_source_edit(section, held, b"after\n", |staged| {
            staged.create_requirement(requirement).map(|_| ())
        })
    });
    drop(reader);

    written.unwrap_or_else(|error| panic!("{error:#}"));
    assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), b"after\n");
    let reopened = Repository::open(&root, "default").unwrap();
    let ids: Vec<_> = reopened
        .store()
        .list_requirements(&ScopeId::new("default").unwrap())
        .unwrap()
        .into_iter()
        .map(|record| record.id.as_str().to_owned())
        .collect();
    assert_eq!(ids, ["req_a"]);
}
