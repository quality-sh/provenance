//! Bead provenance-60t3, wip branch only. These tests show a fault: what a
//! person sees after a second init stopped during the replacement of a
//! project file. No Rule binding: the tests approve nothing.
//!
//! The CLI cannot stop init, so each test makes the disk state by hand. The
//! names and contents are those that the transport test
//! `porcelain::init::lock_tests::project_file_stop_60t3` shows after a real
//! stop at the probe `file_displaced`: the target is away, the old bytes are
//! in `.<name>.provenance-<pid>-0.backup`, and the planned bytes are in
//! `.<name>.provenance-<pid>-0.tmp`. Here the pid is 4242.
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "cli_dictionary/support.rs"]
#[allow(dead_code)]
mod dictionary_support;

const PERSON_AGENTS: &str = "# Team notes\n\nKeep this text of the person.\n";
const PERSON_GITIGNORE: &str = "target/\n.provenance/cache/\n";
const SKILL: &str = ".agents/skills/provenance-shaping/SKILL.md";
const DICTIONARY: &str = ".provenance/state/dictionary.json";

fn provenance(root: &Path, args: &[&str]) -> (i32, String) {
    let asset_dir = root.join("../asset-cache");
    std::fs::create_dir_all(&asset_dir).unwrap();
    std::fs::write(
        asset_dir.join("ASD-STE100_ISSUE9.pdf"),
        dictionary_support::dictionary_pdf(),
    )
    .unwrap();
    let output = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .current_dir(root)
        .env("PROVENANCE_STE100_ASSET_DIR", &asset_dir)
        .env("PROVENANCE_STE100_INDEX_DIR", root.join("../indexes"))
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .stdin(std::process::Stdio::null())
        .args(args)
        .output()
        .unwrap();
    let code = output.status.code().unwrap_or(-1);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("$ provenance {}\n{text}[exit {code}]\n", args.join(" "));
    (code, text)
}

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    eprintln!("$ git {}\n{text}", args.join(" "));
    text
}

/// An initialized repository in Git, with one Requirement, and with
/// AGENTS.md and .gitignore that the person wrote. All is committed.
fn repository() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("repo");
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q"]);
    let init = provenance(&root, &["init", "--disposition-actor-id", "reviewer"]);
    assert_eq!(init.0, 0, "{}", init.1);
    let created = provenance(
        &root,
        &["req_pump", "create", "--type", "requirement", "--statement", "The pump stops."],
    );
    assert_eq!(created.0, 0, "{}", created.1);
    std::fs::write(root.join("AGENTS.md"), PERSON_AGENTS).unwrap();
    std::fs::write(root.join(".gitignore"), PERSON_GITIGNORE).unwrap();
    assert!(root.join(SKILL).exists());
    assert!(root.join(DICTIONARY).exists());
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "initial"]);
    (directory, root)
}

fn leftover(root: &Path, target: &str, kind: &str) -> PathBuf {
    let path = root.join(target);
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    path.parent()
        .unwrap()
        .join(format!(".{name}.provenance-4242-0.{kind}"))
}

/// The state after a stop between the move of the old file to the backup
/// name and the move of the new file in.
fn stop_between_the_moves(root: &Path, target: &str, planned: &[u8]) -> Vec<u8> {
    let old = std::fs::read(root.join(target)).unwrap();
    std::fs::write(leftover(root, target, "tmp"), planned).unwrap();
    std::fs::rename(root.join(target), leftover(root, target, "backup")).unwrap();
    old
}

/// What a person runs next: status, a check, a read, init, and again.
fn next_commands(root: &Path) -> [(i32, String); 4] {
    git(root, &["status", "--short"]);
    let check = provenance(root, &["check"]);
    let read = provenance(root, &["req_pump", "get"]);
    let init = provenance(root, &["init"]);
    git(root, &["status", "--short"]);
    let check_after = provenance(root, &["check"]);
    [check, read, init, check_after]
}

#[test]
fn fault_60t3_agents_md_away_init_writes_a_new_file_without_the_text_of_the_person() {
    let (_directory, root) = repository();
    let planned = format!("{PERSON_AGENTS}\n## Provenance\n\n(planned section)\n");
    let old = stop_between_the_moves(&root, "AGENTS.md", planned.as_bytes());

    let [_check, read, init, _] = next_commands(&root);

    assert_eq!(read.0, 0, "{}", read.1);
    assert_eq!(init.0, 0, "{}", init.1);
    let after = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
    eprintln!("AGENTS.md after init:\n{after}");
    assert!(!after.contains("Keep this text"), "{after}");
    assert_eq!(std::fs::read(leftover(&root, "AGENTS.md", "backup")).unwrap(), old);
    assert!(leftover(&root, "AGENTS.md", "tmp").exists());
}

#[test]
fn fault_60t3_gitignore_away_init_writes_a_new_file_without_the_lines_of_the_person() {
    let (_directory, root) = repository();
    let old = stop_between_the_moves(&root, ".gitignore", b"target/\n.provenance/cache/\n");

    let [_check, _read, init, _] = next_commands(&root);

    assert_eq!(init.0, 0, "{}", init.1);
    let after = std::fs::read_to_string(root.join(".gitignore")).unwrap();
    eprintln!(".gitignore after init:\n{after}");
    assert!(!after.contains("target/"), "{after}");
    assert_eq!(std::fs::read(leftover(&root, ".gitignore", "backup")).unwrap(), old);
}

#[test]
fn fault_60t3_skill_file_away_init_installs_it_again() {
    let (_directory, root) = repository();
    let current = std::fs::read(root.join(SKILL)).unwrap();
    stop_between_the_moves(&root, SKILL, &current);

    let [_check, _read, init, _] = next_commands(&root);

    assert_eq!(init.0, 0, "{}", init.1);
    assert_eq!(std::fs::read(root.join(SKILL)).unwrap(), current);
    assert!(leftover(&root, SKILL, "backup").exists());
}

#[test]
fn fault_60t3_dictionary_reference_away() {
    let (_directory, root) = repository();
    let current = std::fs::read(root.join(DICTIONARY)).unwrap();
    stop_between_the_moves(&root, DICTIONARY, &current);

    let [_check, _read, init, _] = next_commands(&root);

    eprintln!(
        "dictionary.json after init: {:?}",
        std::fs::read_to_string(root.join(DICTIONARY))
    );
    assert_eq!(init.0, 0, "{}", init.1);
    assert!(leftover(&root, DICTIONARY, "backup").exists());
}

/// A stop after the temporary file is written and before the first move: the
/// target is in place, and the temporary file stays.
#[test]
fn stop_before_the_first_move_leaves_only_a_temporary_file() {
    let (_directory, root) = repository();
    std::fs::write(leftover(&root, "AGENTS.md", "tmp"), b"planned\n").unwrap();

    let [_check, _read, init, _] = next_commands(&root);

    assert_eq!(init.0, 0, "{}", init.1);
    let after = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
    assert!(after.starts_with(PERSON_AGENTS), "{after}");
    assert!(leftover(&root, "AGENTS.md", "tmp").exists());
}

/// A stop after the new file moved in and before the removal of the backup:
/// the target has the planned bytes, and the backup stays.
#[test]
fn stop_after_the_second_move_leaves_only_a_backup_file() {
    let (_directory, root) = repository();
    let planned = format!("{PERSON_AGENTS}\n## Provenance\n\n(planned section)\n");
    stop_between_the_moves(&root, "AGENTS.md", planned.as_bytes());
    std::fs::rename(leftover(&root, "AGENTS.md", "tmp"), root.join("AGENTS.md")).unwrap();

    let [_check, _read, init, _] = next_commands(&root);

    assert_eq!(init.0, 0, "{}", init.1);
    let after = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
    assert!(after.contains("Keep this text"), "{after}");
    assert!(leftover(&root, "AGENTS.md", "backup").exists());
}
