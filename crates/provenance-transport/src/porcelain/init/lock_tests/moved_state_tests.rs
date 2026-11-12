//! Init and a writer that takes no lock: the state moves during the hold of
//! init, so the store refuses the manifest and init restores its files.
use super::check_and_write_tests::add_the_target_requirement;
use super::{change_agents, initialized, lock_is_free, options, targets};
use crate::porcelain::init::prepare_init;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::storage::local::LocalStateStorage;
use provenance_store::storage::{Revision, StateStorage as _};
use std::cell::RefCell;
use std::rc::Rc;

/// The revision of the state on disk.
fn revision(layout: &ProvenanceLayout) -> Revision {
    LocalStateStorage::new(layout.clone())
        .read_view()
        .unwrap()
        .revision()
        .clone()
}

/// The one shard of the Requirement of the repository.
fn requirement_shard(layout: &ProvenanceLayout) -> Utf8PathBuf {
    let folder = layout.state_dir().join("scopes/default/requirements");
    let mut shards: Vec<Utf8PathBuf> = folder
        .read_dir_utf8()
        .unwrap()
        .map(|entry| entry.unwrap().into_path())
        .collect();
    assert_eq!(shards.len(), 1, "{shards:?}");
    shards.remove(0)
}

/// A person edits the statement of the Requirement in its shard with a text
/// editor, which takes no lock.
fn edit_by_hand(shard: &Utf8Path) {
    let text = std::fs::read_to_string(shard).unwrap();
    let edited = text.replace("follows the policy", "follows the new policy");
    assert_ne!(edited, text);
    std::fs::write(shard, edited).unwrap();
}

/// A second init writes AGENTS.md and .gitignore again and then publishes
/// the manifest with the reviewer list "new-reviewer". In the hold of init,
/// after the validation, a hand edit changes a shard of the state. The store
/// refuses the manifest because the state moved, and init restores AGENTS.md
/// and .gitignore. The manifest stays old, and the lock is free.
#[test]
#[provenance_macros::verifies("rule_init_apply_rolls_back_owned_changes", examples)]
fn a_state_change_without_the_lock_during_init_refuses_the_manifest_and_restores_the_files() {
    let (_directory, repo, layout) = initialized();
    add_the_target_requirement(&repo).unwrap();
    change_agents(&repo);
    std::fs::write(repo.join(".gitignore"), "target/\n").unwrap();
    let manifest = std::fs::read(layout.manifest_path()).unwrap();
    let plan = prepare_init(&repo, options("new-reviewer")).unwrap();
    let shard = requirement_shard(&layout);
    let before = targets(&repo);
    let revisions = Rc::new(RefCell::new(None));
    let seen = Rc::clone(&revisions);
    let probe_layout = layout.clone();
    let probe_shard = shard.clone();
    crate::test_probes::arm("init_validated", move || {
        let base = revision(&probe_layout);
        edit_by_hand(&probe_shard);
        *seen.borrow_mut() = Some((base, revision(&probe_layout)));
    });

    let result = plan.apply();
    crate::test_probes::disarm("init_validated");

    let (base, moved) = revisions
        .borrow_mut()
        .take()
        .expect("init did not reach the end of its validation");
    let error = result.expect_err("init published the manifest on a moved state");
    assert_eq!(
        format!("{error:#}"),
        format!(
            "the state moved from revision {} to {} before the write",
            base.as_str(),
            moved.as_str()
        )
    );
    assert_eq!(
        std::fs::read(repo.join("AGENTS.md")).unwrap(),
        b"# Team notes\n"
    );
    assert_eq!(
        std::fs::read(repo.join(".gitignore")).unwrap(),
        b"target/\n"
    );
    assert_eq!(std::fs::read(layout.manifest_path()).unwrap(), manifest);
    // Only the hand edit stays: no skill file, dictionary reference,
    // temporary file or backup file of init.
    let mut expected = before;
    expected.insert(
        shard.strip_prefix(&repo).unwrap().to_string(),
        Some(std::fs::read(&shard).unwrap()),
    );
    assert_eq!(targets(&repo), expected);
    assert!(lock_is_free(&layout), "init kept the publication lock");
}
