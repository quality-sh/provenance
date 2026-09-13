use assert_cmd::Command;
use provenance_macros::verifies;
use std::{path::Path, sync::OnceLock};

#[path = "cli_dictionary/support.rs"]
#[allow(dead_code)]
mod dictionary_support;

/// The shared asset cache holds the official asset, so every `init` in this
/// file meets the auto-download on its cache hit and never touches the
/// network. The dictionary index lands beside it in the same temporary tree.
fn offline_dictionary_env() -> (&'static Path, &'static Path) {
    static SEED: OnceLock<(&'static Path, &'static Path)> = OnceLock::new();
    SEED.get_or_init(|| {
        // Leaked on purpose: the seeded cache lives as long as the process.
        let temporary: &mut tempfile::TempDir = Box::leak(Box::new(tempfile::tempdir().unwrap()));
        let assets = temporary.path().join("assets");
        std::fs::create_dir(&assets).unwrap();
        std::fs::write(
            assets.join("ASD-STE100_ISSUE9.pdf"),
            dictionary_support::dictionary_pdf(),
        )
        .unwrap();
        let indexes = temporary.path().join("indexes");
        (Box::leak(Box::new(assets)), Box::leak(Box::new(indexes)))
    })
}

fn init(repo: &Path) -> assert_cmd::assert::Assert {
    init_with(repo, &[])
}

fn init_with(repo: &Path, extra: &[&str]) -> assert_cmd::assert::Assert {
    let mut command = Command::cargo_bin("provenance").unwrap();
    let (assets, indexes) = offline_dictionary_env();
    command
        .env("PROVENANCE_STE100_ASSET_DIR", assets)
        .env("PROVENANCE_STE100_INDEX_DIR", indexes);
    command.args([
        "init",
        "--path",
        repo.to_str().unwrap(),
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ]);
    command.args(extra).assert()
}

#[test]
#[verifies("rule_init_installs_bundled_skills", examples)]
#[verifies("rule_init_owns_agents_provenance_section", examples)]
fn init_prints_a_summary_that_separates_new_from_changed_files() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");

    let stdout = init(&repo).success().get_output().stdout.clone();
    let stdout = String::from_utf8(stdout).unwrap();

    assert!(stdout.contains(&format!(
        "Initialized Provenance for scope \"default\" in {}\n",
        repo.display()
    )));
    assert!(stdout.contains("Never lose the why behind your decisions.\n"));
    assert!(!stdout.contains("\nChanged\n"));
    let new_section = stdout
        .split_once("\nNew\n")
        .expect("a New section")
        .1
        .split_once("\nNext steps\n")
        .expect("next steps follow the inventory")
        .0;
    assert!(new_section.contains("  .provenance/state (manifest for scope \"default\")\n"));
    assert!(new_section.contains("  .agents/skills (added 4 skills)\n"));
    assert!(new_section.contains("  .claude/skills (added 4 links)\n"));
    assert!(new_section.contains("  AGENTS.md (added the Provenance section)\n"));
    assert!(new_section.contains("  .gitignore (added one line)\n"));
    assert!(stdout.contains("  1. provenance prime --quiet\n"));
    assert!(stdout.contains("  2. provenance check --quiet\n"));
    assert!(stdout
        .contains("  3. provenance coverage scan --path . --scope default --validate-rules\n"));
    assert!(stdout.contains("Docs: https://github.com/quality-sh/provenance/tree/main/docs\n"));
    assert!(stdout.contains(
        "Dictionary: Imported the Issue 9 dictionary from the official asset.\n"
    ));
}

#[test]
#[verifies("rule_init_plans_all_project_writes", examples)]
#[verifies("rule_init_owns_agents_provenance_section", examples)]
fn init_on_an_existing_repository_reports_only_the_changed_files() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    init(&repo).success();
    std::fs::write(
        repo.join("AGENTS.md"),
        "# Local instructions\n\nKeep this.\n",
    )
    .unwrap();
    std::fs::write(repo.join(".gitignore"), "target/\n").unwrap();

    let stdout = init_with(&repo, &["--disposition-actor-id", "reviewer"])
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(stdout).unwrap();

    assert!(!stdout.contains("\nNew\n"));
    let (changed_section, _) = stdout
        .split_once("\nChanged\n")
        .expect("a Changed section")
        .1
        .split_once("\n\nNext steps\n")
        .expect("next steps follow the inventory");
    assert_eq!(
        changed_section,
        concat!(
            "  .provenance/state/manifest.json (updated the manifest)\n",
            "  AGENTS.md (added the Provenance section)\n",
            "  .gitignore (added one line)"
        )
    );
}

#[test]
#[verifies("rule_init_installs_bundled_skills", examples)]
#[verifies("rule_init_owns_agents_provenance_section", examples)]
fn a_reinit_that_changes_nothing_prints_one_line() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    let index_dir = temporary.path().join("indexes");
    let pdf = temporary.path().join("issue-9.pdf");
    std::fs::write(&pdf, dictionary_support::dictionary_pdf()).unwrap();

    // First init imports the dictionary, so the re-init below meets every
    // unchanged condition: manifest, section, skills, and dictionary.
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
            "--ste-pdf",
            pdf.to_str().unwrap(),
        ])
        .env("PROVENANCE_STE100_INDEX_DIR", &index_dir)
        .assert()
        .success();

    let stdout = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .env("PROVENANCE_STE100_INDEX_DIR", &index_dir)
        .output()
        .unwrap();
    let stdout = String::from_utf8(stdout.stdout).unwrap();

    assert_eq!(
        stdout,
        format!(
            "Provenance is already set up for scope \"default\" in {}. No change.\n",
            repo.display()
        )
    );
}

#[test]
fn quiet_suppresses_the_whole_summary_and_the_already_line() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");

    let stdout = init_with(&repo, &["--quiet"])
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(stdout.is_empty(), "fresh init printed {stdout:?}");

    let stdout = init_with(&repo, &["--quiet"])
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(stdout.is_empty(), "re-init printed {stdout:?}");
}
