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
}

#[test]
fn symlink_creation_failure_uses_the_preplanned_copy() {
    let directory = tempfile::tempdir().unwrap();
    let mut plan = InstallPlan::build(directory.path(), InstallRequest::Init).unwrap();
    let action = plan.claude.remove(0);

    let (reports, fallback) = action
        .apply_with(|_, _| Err(Error::new(ErrorKind::PermissionDenied, "denied")))
        .unwrap();

    assert!(fallback.unwrap().contains("denied"));
    assert!(!reports.is_empty());
    assert!(directory
        .path()
        .join(".claude/skills/provenance-fork-tournament/SKILL.md")
        .is_file());
}
