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

/// The entries of the transaction directory of the cache under `root`.
fn transactions(root: &Utf8Path) -> Vec<Utf8PathBuf> {
    let directory = ProvenanceLayout::new(root).import_transactions_dir();
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| Utf8PathBuf::from_path_buf(entry.unwrap().path()).unwrap())
        .collect()
}

/// Runs the staged write of `fixture` in a child process. The probe at
/// `phase` ends the process with no unwind, as a kill does.
fn stop_in_child(fixture: &Fixture, phase: &str) {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "repository::stage_recovery_tests::crash_child",
            "--nocapture",
        ])
        .env("PROVENANCE_STAGE_CRASH_ROOT", fixture.root.as_str())
        .env("PROVENANCE_STAGE_CRASH_PHASE", phase)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(86), "{phase}");
}

#[test]
fn crash_child() {
    let Ok(root) = std::env::var("PROVENANCE_STAGE_CRASH_ROOT") else {
        return;
    };
    let phase = std::env::var("PROVENANCE_STAGE_CRASH_PHASE").unwrap();
    let phase = CRASH_PHASES
        .iter()
        .map(|(label, _)| *label)
        .find(|label| *label == phase)
        .expect("known crash phase");
    let repository = open(&ProvenanceLayout::new(Utf8PathBuf::from(root)));
    test_probes::arm(phase, || std::process::exit(86));
    stage_rule(&repository).unwrap();
    panic!("crash phase was not reached");
}

#[test]
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

/// An implementation aid with no Rule: it shows the error of a failed
/// removal, which no Rule states.
#[cfg(unix)]
#[test]
fn a_failed_removal_of_a_stopped_transaction_names_the_step_and_the_path() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = fixture();
    stop_in_child(&fixture, "state_prepared");
    let layout = ProvenanceLayout::new(fixture.root.clone());
    let left = dunce::canonicalize(&transactions(&fixture.root)[0]).unwrap();
    let directory = layout.import_transactions_dir();

    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500)).unwrap();
    let result = crate::publication::with_repository_publication(&layout, |_| Ok(()));
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();

    let error = result.unwrap_err();
    assert_eq!(
        format!("{error:#}"),
        format!(
            "remove leftover import transaction {}: {}",
            left.display(),
            std::io::Error::from_raw_os_error(13)
        )
    );
    assert!(left.is_dir(), "the transaction stays");
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
    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert_eq!(std::fs::read_to_string(outside.join("sentinel")).unwrap(), "keep");
}

#[test]
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
