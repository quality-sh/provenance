//! A view store reads one consistent view of canonical state.
//!
//! `Repository::view` reads the state once, in the publication section. The
//! store it returns answers lists and the manifest from that view, reads no
//! file and refuses every write.

use camino::Utf8Path;
use provenance_core::{ScopeId, StableId};
use provenance_macros::verifies;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::publication::with_publication_guard;
use provenance_store::repository::Repository;
use provenance_store::state_store::{CreateSourceInput, StateStore};

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
        .create_source(source_input("source_policy"))
        .unwrap();
    (dir, repository)
}

fn json<T: serde::Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

fn assert_same_reads(view: &StateStore, live: &StateStore) {
    assert_eq!(
        json(&view.list_sources(&scope()).unwrap()),
        json(&live.list_sources(&scope()).unwrap())
    );
    assert_eq!(
        json(&view.list_requirements(&scope()).unwrap()),
        json(&live.list_requirements(&scope()).unwrap())
    );
    assert_eq!(
        json(&view.list_messages(&scope()).unwrap()),
        json(&live.list_messages(&scope()).unwrap())
    );
    assert_eq!(
        json(&view.manifest().unwrap()),
        json(&live.manifest().unwrap())
    );
    assert_eq!(
        view.list_scope_directories().unwrap(),
        live.list_scope_directories().unwrap()
    );
}

#[test]
#[verifies("rule_canonical_state_is_read_as_one_view", examples)]
fn the_view_lists_what_the_live_store_lists() {
    let (_dir, repository) = repository();
    let view = repository.view().unwrap();
    let sources = view.list_sources(&scope()).unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].id.as_str(), "source_policy");
    assert_same_reads(&view, &repository.store());
}

#[test]
#[verifies("rule_canonical_state_is_read_as_one_view", examples)]
fn the_view_reads_no_file_after_it_is_taken() {
    let (_dir, repository) = repository();
    let view = repository.view().unwrap();
    let before = json(&view.list_sources(&scope()).unwrap());
    std::fs::remove_dir_all(repository.layout().state_dir()).unwrap();
    assert_eq!(json(&view.list_sources(&scope()).unwrap()), before);
    assert_eq!(view.manifest().unwrap().scopes.len(), 1);
}

#[test]
#[verifies("rule_canonical_state_is_read_as_one_view", examples)]
fn the_view_refuses_a_write() {
    let (_dir, repository) = repository();
    let view = repository.view().unwrap();
    let error = view
        .create_source(source_input("source_other"))
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("a view store"),
        "unexpected error: {error:#}"
    );
    assert_eq!(repository.store().list_sources(&scope()).unwrap().len(), 1);
}

#[test]
#[verifies("rule_canonical_state_is_read_as_one_view", examples)]
fn a_handle_that_holds_the_section_takes_its_view_in_that_section() {
    let (_dir, repository) = repository();
    let sources = with_publication_guard(&repository, |guard| {
        let view = guard.repository().view()?;
        view.list_sources(&scope())
    })
    .unwrap();
    assert_eq!(sources.len(), 1);
}
