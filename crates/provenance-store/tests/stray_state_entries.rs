//! An entry under `.provenance/state` that is not the manifest or a shard is
//! not canonical state.
//!
//! A write still lands and reports success, and the view leaves the entry
//! out. A shard that the process cannot read is still an error.
#![cfg(unix)]

use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{ScopeId, StableId};
use provenance_macros::verifies;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::repository::Repository;
use provenance_store::state_store::CreateSourceInput;
use std::os::unix::fs::PermissionsExt;

#[path = "support/initialization.rs"]
mod initialization;

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

fn source_input(id: &str) -> CreateSourceInput {
    CreateSourceInput {
        scope_id: scope(),
        id: StableId::new(id).unwrap(),
        name: id.to_owned(),
        source_type: provenance_core::SourceType::Policy,
        url: None,
        reference: Some("docs/policy.md".to_owned()),
        commit_pin: None,
        effective_date: None,
        review_date: None,
        supersedes: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

/// A repository with one Source, created through the live store.
fn repository() -> (tempfile::TempDir, Repository) {
    let dir = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(dir.path()).unwrap());
    initialization::initialize(&layout, &[]);
    let repository = Repository::open(dir.path(), "default").unwrap();
    repository
        .store()
        .create_source(source_input("source_first"))
        .unwrap();
    (dir, repository)
}

fn sources_dir(repository: &Repository) -> Utf8PathBuf {
    repository.layout().scopes_dir().join("default/sources")
}

/// Makes the file at `path` unreadable. Returns `false` when the process
/// can still read it, for example as root.
fn make_unreadable(path: &Utf8Path) -> bool {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000)).unwrap();
    std::fs::read(path).is_err()
}

/// A write lands and reports success, and the view lists both Sources.
fn assert_write_and_view_work(repository: &Repository) {
    repository
        .store()
        .create_source(source_input("source_second"))
        .unwrap();
    let sources = repository.view().unwrap().list_sources(&scope()).unwrap();
    let ids: Vec<_> = sources.iter().map(|source| source.id.as_str()).collect();
    assert_eq!(ids, ["source_first", "source_second"]);
}

#[cfg(target_os = "linux")]
#[test]
#[verifies("rule_canonical_state_is_read_as_one_view", examples)]
fn a_stray_name_that_is_not_utf8_is_left_out() {
    use std::os::unix::ffi::OsStrExt;
    let (_dir, repository) = repository();
    let name = std::ffi::OsStr::from_bytes(b"stray-\xff.jsonl");
    std::fs::write(sources_dir(&repository).as_std_path().join(name), b"x").unwrap();
    std::fs::write(
        repository.layout().state_dir().as_std_path().join(name),
        b"x",
    )
    .unwrap();
    assert_write_and_view_work(&repository);
}

#[test]
#[verifies("rule_canonical_state_is_read_as_one_view", examples)]
fn a_stray_name_with_a_backslash_is_left_out() {
    let (_dir, repository) = repository();
    std::fs::write(sources_dir(&repository).join("stray\\name.jsonl"), b"x").unwrap();
    std::fs::write(repository.layout().state_dir().join("a\\b"), b"x").unwrap();
    assert_write_and_view_work(&repository);
}

#[test]
#[verifies("rule_canonical_state_is_read_as_one_view", examples)]
fn an_unreadable_stray_file_is_left_out() {
    let (_dir, repository) = repository();
    let stray = sources_dir(&repository).join("notes.txt");
    std::fs::write(&stray, b"x").unwrap();
    if !make_unreadable(&stray) {
        return;
    }
    assert_write_and_view_work(&repository);
}

#[test]
#[verifies("rule_canonical_state_is_read_as_one_view", examples)]
fn an_unreadable_shard_is_an_error() {
    let (_dir, repository) = repository();
    let shard = sources_dir(&repository).join("source.jsonl");
    if !make_unreadable(&shard) {
        return;
    }
    assert!(repository.view().is_err());
}
