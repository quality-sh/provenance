//! Crash and error phases of `Repository::stage`.
//!
//! The commit point is the rename of the staged state into live. An
//! interruption before it reopens as the complete old state; one after it
//! reopens as the complete new state. No row expects a mixture.

use super::Repository;
use crate::cache::tests::fixtures::{open, seeded_layout, sid};
use crate::cache::{catch_up_state, open_cache};
use crate::layout::ProvenanceLayout;
use crate::state_store::{CreateRuleInput, StateStore};
use crate::test_probes;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{RuleSeverity, RuleStatus, ScopeId, Stamp};
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

fn write_rule(staged: &StateStore) -> anyhow::Result<()> {
    staged.create_rule(CreateRuleInput {
        archived_in_commit: None,
        scope_id: scope(),
        id: sid(STAGED_RULE),
        name: None,
        description: None,
        requirement_ids: vec![sid("req_schads_overtime")],
        resolution_ids: vec![sid("res_schads_overtime")],
        statement: "Pay overtime on public holidays".into(),
        status: RuleStatus::Active,
        severity: RuleSeverity::High,
        source_document: None,
        source_section: None,
        origin_thread: None,
        origin_message: None,
    })?;
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
    let expected_serial = if committed {
        fixture.serial + 1
    } else {
        fixture.serial
    };
    assert_eq!(serial(&repository), expected_serial, "{phase}: serial");
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
        assert_reopened(&fixture, committed, phase);
    }
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
