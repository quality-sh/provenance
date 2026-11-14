//! Crash and error phases of `Repository::stage`.
//!
//! The commit point is the rename of the staged state into live. An
//! interruption before it reopens as the complete old state; one after it
//! reopens as the complete new state. No row expects a mixture.

use super::Repository;
use crate::cache::tests::fixtures::{open, seeded_layout, sid};
use crate::cache::{catch_up_state, open_cache};
use crate::layout::ProvenanceLayout;
use crate::state_store::StateStore;
use crate::stopped_child::{names, run_as_child};
use crate::test_probes;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{ScopeId, Stamp};
use provenance_macros::verifies;
use std::collections::BTreeMap;

const STAGED_RULE: &str = "rule_staged_publication";

/// Crash probes in publication order, and whether the reopened state is new.
const CRASH_PHASES: [(&str, bool); 8] = [
    ("state_prepared", false),
    ("state_marker_prepared", false),
    ("state_after_backup_rename", false),
    ("state_backup_created", false),
    ("state_before_install", false),
    ("state_after_install_rename", true),
    ("state_installed", true),
    ("state_published", true),
];

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

fn stamp() -> Stamp {
    serde_json::from_value(serde_json::json!({
        "commit": "a".repeat(40),
        "at": "2026-10-28T00:00:00Z"
    }))
    .unwrap()
}

/// Adds one Rule to the staged Rule shard.
fn write_rule(staged: &StateStore) -> anyhow::Result<()> {
    let path = crate::shards::rules_path(&staged.layout, &scope());
    let mut rules = staged.list_rules(&scope())?;
    let mut rule = rules[0].clone();
    rule.id = sid(STAGED_RULE);
    rule.statement = "Pay overtime on public holidays".into();
    rule.created = None;
    rule.updated = None;
    rules.push(rule);
    staged.replace_graph_records_guarded(&path, rules)?;
    Ok(())
}

fn stage_rule(repository: &Repository) -> anyhow::Result<()> {
    repository.stage(false, Some(stamp()), write_rule)
}

/// Every state file under `root`, by path relative to the state directory.
fn state_tree(root: &Utf8Path) -> BTreeMap<Utf8PathBuf, Vec<u8>> {
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
    let state = ProvenanceLayout::new(root).state_dir();
    let mut tree = BTreeMap::new();
    visit(&state, &state, &mut tree);
    tree
}

fn copy_state(from: &Utf8Path, to: &Utf8Path) {
    let target = ProvenanceLayout::new(to).state_dir();
    for (relative, bytes) in state_tree(from) {
        let path = target.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
}

fn serial(repository: &Repository) -> i64 {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let report = catch_up_state(repository).await.unwrap();
            let cache = open_cache(repository.layout()).await.unwrap();
            cache.close().await.unwrap();
            report.serial
        })
}

/// One fixture with a stored projection, its old state tree and the new
/// state tree that a twin repository publishes with the same stamp.
struct Fixture {
    _dir: tempfile::TempDir,
    _twin: tempfile::TempDir,
    root: Utf8PathBuf,
    old: BTreeMap<Utf8PathBuf, Vec<u8>>,
    new: BTreeMap<Utf8PathBuf, Vec<u8>>,
    serial: i64,
}

fn fixture() -> Fixture {
    let (dir, layout, _scope) = seeded_layout();
    let root = layout.root().to_owned();
    let serial = serial(&open(&layout));
    let old = state_tree(&root);
    let twin = tempfile::tempdir().unwrap();
    let twin_root = Utf8PathBuf::from_path_buf(twin.path().to_path_buf()).unwrap();
    copy_state(&root, &twin_root);
    stage_rule(&open(&ProvenanceLayout::new(twin_root.clone()))).unwrap();
    let new = state_tree(&twin_root);
    assert_ne!(old, new, "the staged write changes the state");
    Fixture {
        _dir: dir,
        _twin: twin,
        root,
        old,
        new,
        serial,
    }
}

fn assert_reopened(fixture: &Fixture, committed: bool, phase: &str) {
    let repository = open(&ProvenanceLayout::new(fixture.root.clone()));
    let expected = if committed {
        &fixture.new
    } else {
        &fixture.old
    };
    assert!(
        state_tree(&fixture.root) == *expected,
        "{phase}: the reopened state is the complete {} state",
        if committed { "new" } else { "old" }
    );
    assert!(
        !repository.layout().publication_marker_path().exists(),
        "{phase}: no publication marker remains"
    );
    let rules = repository.store().list_rules(&scope()).unwrap();
    assert_eq!(
        rules.iter().any(|rule| rule.id.as_str() == STAGED_RULE),
        committed,
        "{phase}: list_rules answers the reopened state"
    );
    assert_eq!(
        transactions(&fixture.root),
        Vec::<Utf8PathBuf>::new(),
        "{phase}: the cache keeps no transaction data"
    );
    let expected_serial = if committed {
        fixture.serial + 1
    } else {
        fixture.serial
    };
    assert_eq!(serial(&repository), expected_serial, "{phase}: serial");
}

/// The entries of the transaction directory of the cache under `root`, in
/// name order.
fn transactions(root: &Utf8Path) -> Vec<Utf8PathBuf> {
    let directory = ProvenanceLayout::new(root).import_transactions_dir();
    names(&directory)
        .iter()
        .map(|name| directory.join(name))
        .collect()
}

/// The test that the stop tests run as a child.
const CHILD: &str = "repository::stage_recovery_tests::crash_child";

/// Runs the staged write of `fixture` in a child process. The probe at
/// `phase` ends the process with no unwind, as a kill does.
fn stop_in_child(fixture: &Fixture, phase: &str) {
    crate::stopped_child::stop_in_child(CHILD, &fixture.root, phase);
}

#[test]
fn crash_child() {
    let phases = CRASH_PHASES.map(|(label, _)| label);
    run_as_child(&phases, |root| {
        stage_rule(&open(&ProvenanceLayout::new(root.to_owned()))).unwrap();
    });
}

#[test]
#[verifies("rule_live_projection_shows_only_published_state", examples)]
#[verifies("rule_stopped_write_leaves_no_transaction_data", examples)]
fn process_crashes_reopen_as_complete_old_or_new_state() {
    for (phase, committed) in CRASH_PHASES {
        let fixture = fixture();
        stop_in_child(&fixture, phase);
        assert_reopened(&fixture, committed, phase);
    }
}

#[test]
#[verifies("rule_stopped_write_leaves_no_transaction_data", examples)]
fn a_stop_before_the_prepared_marker_leaves_a_transaction_that_the_next_section_removes() {
    let fixture = fixture();
    stop_in_child(&fixture, "state_prepared");

    let left = transactions(&fixture.root);
    assert_eq!(left.len(), 1, "the stopped write left its transaction");
    assert!(
        state_tree(&left[0].join("staged-repo")) == fixture.new,
        "the transaction holds a full copy of the state"
    );
    let layout = ProvenanceLayout::new(fixture.root.clone());
    assert!(!layout.publication_marker_path().exists());
    assert_reopened(&fixture, false, "state_prepared");
}

/// The child stops before the Prepared marker. With no write permission on
/// the import transaction directory, the next section entry cannot remove
/// the transaction.
#[cfg(unix)]
#[test]
#[verifies("rule_failed_removal_of_stopped_write_names_its_path", examples)]
fn a_failed_removal_of_a_stopped_transaction_names_the_step_and_the_path() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = fixture();
    stop_in_child(&fixture, "state_prepared");
    let layout = ProvenanceLayout::new(fixture.root.clone());
    let left = dunce::canonicalize(&transactions(&fixture.root)[0]).unwrap();
    let directory = layout.import_transactions_dir();

    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500)).unwrap();
    let mut ran = false;
    let result = crate::publication::with_repository_publication(&layout, |_| {
        ran = true;
        Ok(())
    });
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();

    let error = crate::write_error::WriteError(result.unwrap_err());
    assert!(matches!(
        error.safe(),
        crate::write_error::WriteFailure::WriteFailed { .. }
    ));
    assert_eq!(
        format!("{:#}", error.0),
        format!(
            "remove transaction directory {} (no marker names this directory, thus the store \
             does not use its data, and a person can remove it): {}",
            left.display(),
            std::io::Error::from_raw_os_error(13)
        )
    );
    assert!(!ran, "the section entry does not run the operation");
    crate::publication::with_repository_publication(&layout, |_| Ok(())).unwrap();
    assert_eq!(transactions(&fixture.root), Vec::<Utf8PathBuf>::new());
    assert!(
        state_tree(&fixture.root) == fixture.old,
        "the live state does not change"
    );
}

/// A link in the transaction directory is an entry that no write makes, so
/// the test puts it there itself.
#[cfg(unix)]
#[test]
#[verifies("rule_recovery_stays_in_cache", examples)]
fn the_removal_of_a_stopped_transaction_keeps_a_link_and_its_target() {
    let fixture = fixture();
    let layout = ProvenanceLayout::new(fixture.root.clone());
    let outside = fixture.root.join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("sentinel"), "keep").unwrap();
    std::fs::create_dir_all(layout.import_transactions_dir()).unwrap();
    let link = layout.import_transactions_dir().join("link");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    stop_in_child(&fixture, "state_prepared");

    crate::publication::with_repository_publication(&layout, |_| Ok(())).unwrap();

    assert_eq!(transactions(&fixture.root), vec![link.clone()]);
    assert!(std::fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        std::fs::read_to_string(outside.join("sentinel")).unwrap(),
        "keep"
    );
}

#[test]
#[verifies("rule_live_projection_shows_only_published_state", examples)]
fn errors_before_published_roll_back_and_an_error_at_published_keeps_new_state() {
    for (phase, _) in CRASH_PHASES {
        let fixture = fixture();
        let repository = open(&ProvenanceLayout::new(fixture.root.clone()));
        test_probes::crash_at(phase);
        let result = stage_rule(&repository);
        test_probes::disarm(phase);
        assert!(
            result.is_err(),
            "{phase}: the probe error reaches the caller"
        );
        assert_reopened(&fixture, phase == "state_published", phase);
    }
}

/// The write stops after its Published marker. With no write permission on
/// the import transaction directory, the next section entry removes the
/// contents of the transaction and then fails to remove the transaction.
#[cfg(unix)]
#[test]
#[verifies("rule_stopped_write_leaves_no_transaction_data", examples)]
#[verifies("rule_failed_removal_of_stopped_write_names_its_path", examples)]
fn a_section_entry_ends_a_published_import_whose_transaction_is_removed_in_part() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = fixture();
    stop_in_child(&fixture, "state_published");
    let layout = ProvenanceLayout::new(fixture.root.clone());
    let transaction = transactions(&fixture.root)[0].clone();
    let directory = layout.import_transactions_dir();

    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500)).unwrap();
    let failed = crate::publication::with_repository_publication(&layout, |_| Ok(()));
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
    let error = crate::write_error::WriteError(failed.unwrap_err());
    assert!(matches!(
        error.safe(),
        crate::write_error::WriteFailure::WriteFailed { .. }
    ));
    assert_eq!(
        format!("{:#}", error.0),
        format!(
            "remove transaction directory {}: {}",
            dunce::canonicalize(&transaction).unwrap().display(),
            std::io::Error::from_raw_os_error(13)
        )
    );
    assert_eq!(
        std::fs::read_dir(&transaction).unwrap().count(),
        0,
        "a part is removed"
    );
    assert!(layout.publication_marker_path().exists());

    assert_reopened(&fixture, true, "state_published");
}

/// The probe after the Published marker takes the write permission from the
/// import transaction directory, so the removal of the transaction removes
/// its contents and then fails.
#[cfg(unix)]
#[test]
#[verifies("rule_published_write_succeeds_when_its_cleanup_fails", examples)]
fn a_published_import_answers_success_when_the_removal_of_its_transaction_fails() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = fixture();
    let layout = ProvenanceLayout::new(fixture.root.clone());
    let directory = layout.import_transactions_dir();
    let locked = directory.clone();
    test_probes::arm("state_published", move || {
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500))?;
        Ok(())
    });

    let answer = stage_rule(&open(&layout));
    test_probes::disarm("state_published");
    let left = transactions(&fixture.root);
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();

    answer.unwrap();
    assert_eq!(left.len(), 1, "the transaction stays");
    assert_eq!(names(&left[0]), Vec::<String>::new());
    assert!(layout.publication_marker_path().exists());
    assert_reopened(&fixture, true, "state_published");
}

/// The child stops in the write of the Prepared marker: after the probe
/// before that write, the system ends the child at the first byte of the
/// temporary file of the marker.
#[cfg(target_os = "linux")]
#[test]
#[verifies("rule_stopped_write_leaves_no_transaction_data", examples)]
fn a_stop_in_the_write_of_the_import_marker_leaves_no_file_after_the_next_section_entry() {
    let fixture = fixture();
    let layout = ProvenanceLayout::new(fixture.root.clone());
    let marker_temporaries = || {
        crate::stopped_child::files_named(&layout.cache_dir(), ".import-publication.json.", ".tmp")
    };
    assert_eq!(marker_temporaries(), Vec::<String>::new());

    crate::stopped_child::stop_in_child_at_file_size(CHILD, &fixture.root, "state_prepared", 0);

    assert_eq!(
        marker_temporaries().len(),
        1,
        "the stop left the temporary file of the marker"
    );
    assert!(!layout.publication_marker_path().exists());
    crate::publication::with_repository_publication(&layout, |_| Ok(())).unwrap();
    assert_eq!(marker_temporaries(), Vec::<String>::new());
    assert_reopened(&fixture, false, "the write of the Prepared marker");
}
