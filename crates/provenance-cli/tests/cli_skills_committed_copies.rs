//! Guards the three committed copies of each bundled skill against drift.
//!
//! The repo commits every bundled skill three times: the source in `skills/`,
//! and two installed copies in `.agents/skills` and `.claude/skills`. The
//! installed copies carry an ownership stamp that the source does not. An
//! edit to one copy can leave the other two behind.
//!
//! This test does not compare the three files with each other, and it does not
//! know how a stamp is built. It runs an install into a temporary directory,
//! then compares the result with the committed copies. The installer compiles
//! the `skills/` source into the binary with `include_str!`, so a match proves
//! that all three copies agree. The check therefore cannot drift away from
//! install semantics.
//!
//! The stamp holds the crate version, so a version bump makes this test fail
//! until the installed copies are written again. The failure message gives the
//! command that does it.

use assert_cmd::Command;
use std::path::{Path, PathBuf};

/// The two directories that an install writes a stamped copy into.
const INSTALLED_ROOTS: [&str; 2] = [".agents/skills", ".claude/skills"];

/// `.claude/skills` also holds skills that provenance does not bundle, so the
/// binary's own list is what this test examines. Anything else in those
/// directories stays untouched.
#[test]
fn committed_skill_copies_match_what_an_install_writes() {
    let workspace = workspace_root();
    let installed = tempfile::tempdir().unwrap();

    // `--copy` puts a real file in `.claude/skills`, which is the form the
    // repo commits. A default install would put a symlink there instead.
    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(installed.path())
        .args(["skills", "install", "--copy", "--format", "json"])
        .assert()
        .success();

    let names = bundled_skill_names();
    assert!(!names.is_empty(), "the binary bundles no skills");

    for name in &names {
        for root in INSTALLED_ROOTS {
            let relative = Path::new(root).join(name).join("SKILL.md");
            let expected = std::fs::read_to_string(installed.path().join(&relative)).unwrap();
            let committed = std::fs::read_to_string(workspace.join(&relative))
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", relative.display()));

            assert!(
                committed == expected,
                "{} is not what an install writes: {}.\n\
                 Run `provenance skills install --copy --force` in the repo root, \
                 then commit the result.",
                relative.display(),
                first_difference(&committed, &expected)
            );
        }
    }
}

/// The skills that the binary bundles, named as the binary itself names them.
fn bundled_skill_names() -> Vec<String> {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args(["skills", "list", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let listed: Vec<serde_json::Value> = serde_json::from_slice(&output).unwrap();
    listed
        .iter()
        .map(|skill| skill["name"].as_str().unwrap().to_string())
        .collect()
}

/// Where two files first differ. A failure names the place, because a whole
/// skill file is too large to print.
fn first_difference(committed: &str, expected: &str) -> String {
    for (offset, (left, right)) in committed.lines().zip(expected.lines()).enumerate() {
        if left != right {
            return format!(
                "line {} is {:?}, but an install writes {:?}",
                offset + 1,
                shorten(left),
                shorten(right)
            );
        }
    }

    let committed_lines = committed.lines().count();
    let expected_lines = expected.lines().count();
    if committed_lines == expected_lines {
        return "the two differ in trailing characters only".to_string();
    }
    format!(
        "the committed copy has {committed_lines} lines, but an install writes {expected_lines}"
    )
}

/// At most 120 characters of `line`, so that one long description cannot fill
/// the log.
fn shorten(line: &str) -> String {
    const LIMIT: usize = 120;
    if line.chars().count() <= LIMIT {
        return line.to_string();
    }
    format!("{}...", line.chars().take(LIMIT).collect::<String>())
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
