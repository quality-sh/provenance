use super::*;
use std::io::{Error, ErrorKind};

#[test]
fn legacy_parent_change_refuses_the_planned_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    InstallPlan::build(
        directory.path(),
        InstallRequest::Standalone {
            global: false,
            force: false,
            copy: false,
        },
    )
    .unwrap()
    .apply()
    .unwrap();
    let legacy = directory.path().join(".agents/skills/shaping");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::copy(
        directory
            .path()
            .join(".agents/skills/provenance-shaping/SKILL.md"),
        legacy.join("SKILL.md"),
    )
    .unwrap();
    let canonical = directory
        .path()
        .join(".agents/skills/provenance-shaping/SKILL.md");
    let prior = std::fs::read_to_string(&canonical)
        .unwrap()
        .replace(env!("CARGO_PKG_VERSION"), "0.2.1");
    std::fs::write(&canonical, &prior).unwrap();
    std::fs::remove_dir_all(directory.path().join(".claude")).unwrap();
    let plan = InstallPlan::build(directory.path(), InstallRequest::Init).unwrap();
    std::fs::write(legacy.join("user.txt"), "user\n").unwrap();

    let Err(error) = plan.apply() else {
        panic!("stale cleanup plan succeeded");
    };

    assert!(error
        .to_string()
        .contains("changed after skill cleanup was planned"));
    assert!(legacy.join("SKILL.md").exists());
    assert!(legacy.join("user.txt").exists());
    assert_eq!(std::fs::read_to_string(canonical).unwrap(), prior);
    assert!(!directory.path().join(".claude").exists());
}

#[test]
fn symlink_creation_failure_uses_the_preplanned_copy() {
    let directory = tempfile::tempdir().unwrap();
    let mut plan = InstallPlan::build(directory.path(), InstallRequest::Init).unwrap();
    let action = plan.claude.remove(0);
    let mut rollback = FileRollbackJournal::default();

    let (reports, fallback) = action
        .apply_with(
            |_, _| Err(Error::new(ErrorKind::PermissionDenied, "denied")),
            &mut rollback,
        )
        .unwrap();

    assert!(fallback.unwrap().contains("denied"));
    assert!(!reports.is_empty());
    assert!(directory
        .path()
        .join(".claude/skills/provenance-fork-tournament/SKILL.md")
        .is_file());
}

#[test]
#[provenance_macros::verifies("rule_init_apply_rolls_back_owned_changes", examples)]
fn rollback_restores_a_force_displaced_claude_directory() {
    let directory = tempfile::tempdir().unwrap();
    let displaced = directory
        .path()
        .join(".claude/skills/provenance-fork-tournament");
    std::fs::create_dir_all(&displaced).unwrap();
    std::fs::write(displaced.join("user.txt"), "preserve me\n").unwrap();
    let mut plan = InstallPlan::build(
        directory.path(),
        InstallRequest::Standalone {
            global: false,
            force: true,
            copy: false,
        },
    )
    .unwrap();
    let action = plan.claude.remove(0);
    let mut rollback = FileRollbackJournal::within(directory.path());

    action.apply(&mut rollback).unwrap();
    rollback.rollback().unwrap();

    assert_eq!(
        std::fs::read_to_string(displaced.join("user.txt")).unwrap(),
        "preserve me\n"
    );
    assert!(!displaced.join("SKILL.md").exists());
}

#[test]
fn force_displacement_refuses_same_kind_content_changed_after_planning() {
    let directory = tempfile::tempdir().unwrap();
    let displaced = directory
        .path()
        .join(".claude/skills/provenance-fork-tournament");
    std::fs::create_dir_all(&displaced).unwrap();
    std::fs::write(displaced.join("user.txt"), "planned\n").unwrap();
    let mut plan = InstallPlan::build(
        directory.path(),
        InstallRequest::Standalone {
            global: false,
            force: true,
            copy: false,
        },
    )
    .unwrap();
    let action = plan.claude.remove(0);
    std::fs::write(displaced.join("user.txt"), "concurrent\n").unwrap();
    let mut rollback = FileRollbackJournal::within(directory.path());

    let Err(error) = action.apply(&mut rollback) else {
        panic!("stale forced replacement succeeded");
    };

    assert!(format!("{error:#}").contains("target contents changed"));
    assert_eq!(
        std::fs::read_to_string(displaced.join("user.txt")).unwrap(),
        "concurrent\n"
    );
}
