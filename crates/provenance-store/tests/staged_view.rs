//! A staged write is a view of the live repository.
//!
//! Inside `Repository::stage` the store keeps the live identity and reads and
//! writes the staged state root. The body cannot reach the live state, and
//! live access works again after the stage.

use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{Manifest, RepoPathPrefix, ScopeId, Source, StableId};
use provenance_macros::verifies;
use provenance_store::cache::catch_up_state;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use provenance_store::publication::{with_publication_guard, LiveAccessDuringStage};
use provenance_store::repository::{OpenFailure, Repository};
use provenance_store::state_store::{CreateSourceInput, ScopeShards, StateStore};
use std::collections::BTreeMap;
use std::sync::mpsc;
use std::time::Duration;

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
        reference: None,
        commit_pin: None,
        effective_date: None,
        review_date: None,
        supersedes: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

/// A repository whose live root alone holds `settings.json`, with one Source.
fn repository() -> (tempfile::TempDir, Repository) {
    let dir = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(dir.path()).unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&Manifest::default_with_scope(
            scope(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        layout.provenance_dir().join("settings.json"),
        r#"{"read":{"freshness_policy":"refuse_stale"}}"#,
    )
    .unwrap();
    let repository = Repository::open(dir.path(), "default").unwrap();
    repository
        .store()
        .create_source(source_input("source_live"))
        .unwrap();
    (dir, repository)
}

/// Adds one Source to the staged scope through the scope import.
fn add_source(staged: &StateStore, id: &str) -> anyhow::Result<()> {
    let mut sources: Vec<Source> = staged.list_sources(&scope())?;
    let mut added = sources[0].clone();
    added.id = StableId::new(id)?;
    added.name = id.to_owned();
    sources.push(added);
    staged.import_scope(
        &scope(),
        &ScopeShards {
            sources: &sources,
            ..ScopeShards::default()
        },
    )
}

fn state_bytes(repository: &Repository) -> BTreeMap<Utf8PathBuf, Vec<u8>> {
    fn visit(base: &Utf8Path, directory: &Utf8Path, tree: &mut BTreeMap<Utf8PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = Utf8PathBuf::from_path_buf(entry.unwrap().path()).unwrap();
            if path.is_dir() {
                visit(base, &path, tree);
            } else {
                let relative = path.strip_prefix(base).unwrap().to_owned();
                tree.insert(relative, std::fs::read(&path).unwrap());
            }
        }
    }
    let state = repository.layout().state_dir();
    let mut tree = BTreeMap::new();
    visit(&state, &state, &mut tree);
    tree
}

fn serial(repository: &Repository) -> i64 {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(catch_up_state(repository))
        .unwrap()
        .serial
}

fn transactions(repository: &Repository) -> Vec<Utf8PathBuf> {
    let dir = repository.layout().import_transactions_dir();
    if !dir.exists() {
        return Vec::new();
    }
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| Utf8PathBuf::from_path_buf(entry.unwrap().path()).unwrap())
        .collect()
}

fn has_source(repository: &Repository, id: &str) -> bool {
    repository
        .store()
        .list_sources(&scope())
        .unwrap()
        .iter()
        .any(|source| source.id.as_str() == id)
}

fn refused_during_stage(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.downcast_ref::<LiveAccessDuringStage>().is_some())
}

#[test]
#[verifies("rule_staged_write_keeps_live_repository_identity", examples)]
fn the_staged_store_reports_the_live_identity() {
    let (_dir, repository) = repository();
    repository
        .stage(true, None, |staged| {
            let identity = staged.repository().expect("a staged store has an identity");
            assert_eq!(identity.working_tree(), repository.working_tree());
            assert_eq!(identity.scope(), repository.scope());
            assert_eq!(
                ReadPolicy::resolve(identity.settings(), None).freshness,
                FreshnessPolicy::RefuseStale,
                "the settings come from the live root"
            );
            Ok(())
        })
        .unwrap();
}

#[test]
#[verifies("rule_staged_write_keeps_live_repository_identity", examples)]
fn a_write_in_the_body_reaches_live_state_only_when_the_stage_returns() {
    let (_dir, repository) = repository();
    let before = state_bytes(&repository);
    let serial_before = serial(&repository);
    repository
        .stage(false, None, |staged| {
            add_source(staged, "source_staged")?;
            assert!(staged
                .list_sources(&scope())?
                .iter()
                .any(|source| source.id.as_str() == "source_staged"));
            assert_eq!(state_bytes(&repository), before, "live bytes before return");
            for transaction in transactions(&repository) {
                assert!(
                    !transaction.join("staged-repo/.provenance/cache").exists(),
                    "the staged copy takes no lock and gets no cache"
                );
            }
            Ok(())
        })
        .unwrap();
    assert!(has_source(&repository, "source_staged"));
    assert_eq!(serial(&repository), serial_before + 1);
}

#[test]
#[verifies("rule_staged_write_keeps_live_repository_identity", examples)]
fn a_dry_run_leaves_live_state_and_the_projection_unchanged() {
    let (_dir, repository) = repository();
    let serial_before = serial(&repository);
    let before = state_bytes(&repository);
    let transactions_before = transactions(&repository);
    repository
        .stage(true, None, |staged| add_source(staged, "source_dry"))
        .unwrap();
    assert_eq!(state_bytes(&repository), before);
    assert_eq!(transactions(&repository), transactions_before);
    assert_eq!(serial(&repository), serial_before);
}

#[test]
#[verifies("rule_staged_write_keeps_live_repository_identity", examples)]
fn a_body_error_after_a_write_leaves_live_state_unchanged() {
    let (_dir, repository) = repository();
    let serial_before = serial(&repository);
    let before = state_bytes(&repository);
    let error = repository
        .stage(false, None, |staged| {
            add_source(staged, "source_failed")?;
            anyhow::bail!("the body refuses")
        })
        .unwrap_err();
    assert_eq!(error.to_string(), "the body refuses");
    assert_eq!(state_bytes(&repository), before);
    assert!(!repository.layout().publication_marker_path().exists());
    assert!(transactions(&repository).is_empty());
    assert_eq!(serial(&repository), serial_before);
}

#[test]
#[verifies("rule_staged_write_keeps_live_repository_identity", examples)]
fn a_live_store_on_another_thread_waits_for_the_stage() {
    let (_dir, repository) = repository();
    let (sender, receiver) = mpsc::channel();
    let waiter = repository.clone();
    let mut handle = None;
    repository
        .stage(false, None, |staged| {
            add_source(staged, "source_waited")?;
            handle = Some(std::thread::spawn(move || {
                let found = has_source(&waiter, "source_waited");
                sender.send(found).unwrap();
            }));
            assert_eq!(
                receiver.recv_timeout(Duration::from_millis(300)),
                Err(mpsc::RecvTimeoutError::Timeout),
                "a live read waits while the stage holds the lock"
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(receiver.recv_timeout(Duration::from_secs(5)), Ok(true));
    handle.unwrap().join().unwrap();
}

#[test]
#[verifies("rule_staged_store_lives_only_inside_stage", examples)]
fn live_access_in_the_body_refuses_and_works_again_after_the_stage() {
    let (_dir, repository) = repository();
    let before = state_bytes(&repository);
    let error = repository
        .stage(false, None, |_staged| {
            let read = repository.store().list_sources(&scope()).unwrap_err();
            assert!(refused_during_stage(&read), "live read: {read:#}");
            let write = repository
                .store()
                .create_source(source_input("source_live_write"))
                .unwrap_err();
            assert!(refused_during_stage(&write), "live write: {write:#}");
            let open = Repository::open(repository.working_tree(), "default").unwrap_err();
            assert!(
                matches!(&open, OpenFailure::Io(error) if refused_during_stage(error)),
                "open: {open}"
            );
            let guard = with_publication_guard(&repository, |_| Ok(())).unwrap_err();
            assert!(refused_during_stage(&guard), "guard: {guard:#}");
            anyhow::bail!("refused body")
        })
        .unwrap_err();
    assert_eq!(error.to_string(), "refused body");
    assert_eq!(state_bytes(&repository), before, "live bytes unchanged");

    repository
        .store()
        .create_source(source_input("source_after_refusal"))
        .unwrap();
    repository
        .stage(false, None, |staged| add_source(staged, "source_staged"))
        .unwrap();
    repository
        .store()
        .create_source(source_input("source_after_stage"))
        .unwrap();
    assert!(has_source(&repository, "source_after_refusal"));
    assert!(has_source(&repository, "source_staged"));
    assert!(has_source(&repository, "source_after_stage"));
}
