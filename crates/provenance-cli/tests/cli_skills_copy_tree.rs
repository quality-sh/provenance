//! The copy path installs the full skill directory.

use assert_cmd::Command;
use std::path::Path;

/// A skill is a directory, not one file: the agent skills specification lets
/// it carry `scripts/`, `references/` and `assets/` beside its `SKILL.md`.
/// The copy path must bring all of that content to `.claude/skills`, and must
/// keep the ownership stamp on the `SKILL.md` that it copies.
#[test]
fn skills_install_copy_brings_the_whole_skill_directory() {
    let dir = tempfile::tempdir().unwrap();
    let skill = "provenance-shaping";
    install(dir.path(), &["--copy"]).success();

    let canonical = dir.path().join(".agents/skills").join(skill);
    std::fs::create_dir_all(canonical.join("scripts")).unwrap();
    std::fs::write(canonical.join("scripts/run.sh"), "#!/bin/sh\necho run\n").unwrap();
    std::fs::create_dir_all(canonical.join("references/deep")).unwrap();
    std::fs::write(canonical.join("references/deep/notes.md"), "notes\n").unwrap();

    install(dir.path(), &["--copy"]).success();

    let copied = dir.path().join(".claude/skills").join(skill);
    assert_eq!(
        std::fs::read_to_string(copied.join("scripts/run.sh")).unwrap(),
        "#!/bin/sh\necho run\n"
    );
    assert_eq!(
        std::fs::read_to_string(copied.join("references/deep/notes.md")).unwrap(),
        "notes\n"
    );

    let stamped = std::fs::read_to_string(canonical.join("SKILL.md")).unwrap();
    assert!(stamped.contains("Installed by provenance"));
    assert_eq!(
        std::fs::read_to_string(copied.join("SKILL.md")).unwrap(),
        stamped
    );
}

/// The installer embeds only the `SKILL.md` of each bundled skill, thus a
/// second file below `skills/` reaches no user. This test fails on such a
/// file, as a reminder to embed it before the skill ships.
#[test]
fn bundled_skills_carry_nothing_that_install_would_drop() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    for skill in std::fs::read_dir(workspace.join("skills")).unwrap() {
        let skill = skill.unwrap();
        if !skill.file_type().unwrap().is_dir() {
            continue;
        }
        let mut names = std::fs::read_dir(skill.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            ["SKILL.md"],
            "{} carries content that install does not embed",
            skill.path().display()
        );
    }
}

/// Runs `provenance skills install` in `dir`, always with the JSON report.
fn install(dir: &Path, arguments: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(dir)
        .args(["skills", "install"])
        .args(arguments)
        .args(["--format", "json"])
        .assert()
}
