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
    *SEED.get_or_init(|| {
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
    assert!(stdout.contains(
        "Provenance records requirements, decisions, and the rules that connect them to code.\n"
    ));
    assert!(!stdout.contains("\nChanged\n"));
    let new_section = stdout
        .split_once("\nNew\n")
        .expect("a New section")
        .1
        .split_once("\nHave your agent run provenance prime to get acclimated.\n")
        .expect("the agent handoff follows the inventory")
        .0;
    assert!(new_section.contains("  .provenance/state (manifest for scope \"default\")\n"));
    assert!(
        new_section.contains("  .agents/skills/provenance-shaping/SKILL.md (added skill file)\n")
    );
    assert!(new_section.contains("  .claude/skills/provenance-shaping (added link)\n"));
    assert!(new_section.contains("  AGENTS.md (added the Provenance section)\n"));
    assert!(new_section.contains("  .gitignore (added one line)\n"));
    assert!(!stdout.contains("Next steps"));
    assert!(!stdout.contains("Dictionary:"));
    assert!(!stdout.contains("official asset"));
    assert!(stdout.ends_with("Have your agent run provenance prime to get acclimated.\n"));
    assert!(repo.join(".provenance/state/dictionary.json").exists());
}

#[test]
fn existing_state_directory_reports_new_manifest_file_and_keeps_other_files() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    let state = repo.join(".provenance/state");
    std::fs::create_dir_all(&state).unwrap();
    let sentinel = state.join("unrelated.txt");
    std::fs::write(&sentinel, "keep this\n").unwrap();

    let stdout = init(&repo).success().get_output().stdout.clone();
    let stdout = String::from_utf8(stdout).unwrap();
    assert!(stdout.contains(
        "  .provenance/state/manifest.json (manifest for scope \"default\")\n"
    ));
    assert!(!stdout.contains("  .provenance/state ("));
    assert!(state.join("manifest.json").is_file());
    assert_eq!(std::fs::read_to_string(&sentinel).unwrap(), "keep this\n");

    let manifest = std::fs::read(state.join("manifest.json")).unwrap();
    let second = init(&repo).success().get_output().stdout.clone();
    let second = String::from_utf8(second).unwrap();
    assert!(second.contains("No change.\n"));
    assert!(!second.contains("\nNew\n"));
    assert!(!second.contains("\nChanged\n"));
    assert_eq!(std::fs::read(state.join("manifest.json")).unwrap(), manifest);
    assert_eq!(std::fs::read_to_string(&sentinel).unwrap(), "keep this\n");
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
        .split_once("\n\nHave your agent run provenance prime to get acclimated.\n")
        .expect("the handoff follows the inventory");
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
            "Provenance is already set up for scope \"default\" in {}. No change.\n\nProvenance records requirements, decisions, and the rules that connect them to code.\n\nHave your agent run provenance prime to get acclimated.\n",
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

#[test]
fn failed_dictionary_acquisition_keeps_init_successful_and_warns_even_when_quiet() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    let asset_dir = temporary.path().join("assets");
    let index_dir = temporary.path().join("indexes");
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--quiet",
        ])
        .env("PROVENANCE_STE100_ASSET_DIR", &asset_dir)
        .env("PROVENANCE_STE100_INDEX_DIR", &index_dir)
        .env(
            "PROVENANCE_TEST_STE100_ASSET_URL",
            "http://127.0.0.1:9/unavailable",
        )
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let warning = String::from_utf8(output.stderr).unwrap();
    assert!(warning.contains("Warning: the official Issue 9 asset is unavailable"));
    assert!(warning.contains("provenance dictionary import --pdf <path>"));
    assert!(repo.join(".provenance/state/manifest.json").exists());
    assert!(!repo.join(".provenance/state/dictionary.json").exists());
}

#[test]
fn dictionary_warning_precedes_the_final_handoff_in_a_combined_stream() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    let output_path = temporary.path().join("terminal.log");
    let output = std::fs::File::create(&output_path).unwrap();
    let errors = output.try_clone().unwrap();
    let status = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
        ])
        .env(
            "PROVENANCE_STE100_ASSET_DIR",
            temporary.path().join("assets"),
        )
        .env(
            "PROVENANCE_STE100_INDEX_DIR",
            temporary.path().join("indexes"),
        )
        .env(
            "PROVENANCE_TEST_STE100_ASSET_URL",
            "http://127.0.0.1:9/unavailable",
        )
        .stdout(output)
        .stderr(errors)
        .status()
        .unwrap();
    assert!(status.success());
    let terminal = std::fs::read_to_string(output_path).unwrap();
    assert!(terminal.contains("Warning: the official Issue 9 asset is unavailable"));
    assert!(terminal.ends_with("Have your agent run provenance prime to get acclimated.\n"));
}

#[test]
fn existing_skill_parents_report_changed_child_paths_and_keep_unrelated_skills() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    for parent in [".agents/skills", ".claude/skills"] {
        let unrelated = repo.join(parent).join("unrelated/SKILL.md");
        std::fs::create_dir_all(unrelated.parent().unwrap()).unwrap();
        std::fs::write(unrelated, "keep this\n").unwrap();
    }
    let output = init(&repo).success().get_output().stdout.clone();
    let output = String::from_utf8(output).unwrap();
    assert!(!output.contains("  .agents/skills ("));
    assert!(!output.contains("  .claude/skills ("));
    assert!(output.contains("  .agents/skills/provenance-shaping/SKILL.md ("));
    assert!(output.contains("  .claude/skills/provenance-shaping ("));
    for parent in [".agents/skills", ".claude/skills"] {
        assert_eq!(
            std::fs::read_to_string(repo.join(parent).join("unrelated/SKILL.md")).unwrap(),
            "keep this\n"
        );
    }
}

#[test]
fn adding_a_dictionary_reference_on_reinit_is_reported_as_a_change() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    let asset_dir = temporary.path().join("assets");
    let index_dir = temporary.path().join("indexes");
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
        ])
        .env("PROVENANCE_STE100_ASSET_DIR", &asset_dir)
        .env("PROVENANCE_STE100_INDEX_DIR", &index_dir)
        .env(
            "PROVENANCE_TEST_STE100_ASSET_URL",
            "http://127.0.0.1:9/unavailable",
        )
        .assert()
        .success();
    let pdf = temporary.path().join("issue-9.pdf");
    std::fs::write(&pdf, dictionary_support::dictionary_pdf()).unwrap();

    let stdout = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--ste-pdf",
            pdf.to_str().unwrap(),
        ])
        .env("PROVENANCE_STE100_INDEX_DIR", &index_dir)
        .output()
        .unwrap();
    assert!(stdout.status.success());
    let stdout = String::from_utf8(stdout.stdout).unwrap();
    assert!(
        stdout.contains("  .provenance/state/dictionary.json (added the dictionary reference)\n")
    );
    assert!(!stdout.contains("No change."));
    assert!(stdout.ends_with("Have your agent run provenance prime to get acclimated.\n"));
}
