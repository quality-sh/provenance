use assert_cmd::Command;
use predicates::prelude::*;
use provenance_macros::verifies;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[test]
fn skills_list_and_show_embedded_skill_files() {
    let skills = workspace_skill_files();

    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args(["skills", "list", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let listed: Vec<serde_json::Value> = serde_json::from_slice(&output).unwrap();
    let listed_names = listed
        .iter()
        .map(|skill| skill["name"].as_str().unwrap().to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(listed_names, skills.keys().cloned().collect());
    assert!(listed.iter().all(|skill| skill["description"]
        .as_str()
        .is_some_and(|description| !description.is_empty())));

    for (name, contents) in skills {
        Command::cargo_bin("provenance")
            .unwrap()
            .args(["skills", "show", &name])
            .assert()
            .success()
            .stdout(contents);
    }
}

#[test]
fn embedded_skills_include_turn_based_provenance_shaping_skill() {
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["skills", "list", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""name": "provenance-shaping""#));

    Command::cargo_bin("provenance")
        .unwrap()
        .args(["skills", "show", "provenance-shaping"])
        .assert()
        .success()
        .stdout(predicate::str::contains("LAND-AS-YOU-GO"))
        .stdout(predicate::str::contains("Chart"))
        .stdout(predicate::str::contains("Work"));
}

#[test]
fn skills_install_default_writes_canonical_files_and_relative_claude_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let skill = "provenance-fork-tournament";

    install(dir.path(), &[])
        .success()
        .stdout(predicate::str::contains(r#""link_mode": "symlink""#));

    let canonical = dir
        .path()
        .join(".agents/skills")
        .join(skill)
        .join("SKILL.md");
    let canonical_contents = std::fs::read_to_string(&canonical).unwrap();
    assert!(canonical_contents.starts_with("---\nname: provenance-fork-tournament"));
    assert!(canonical_contents.contains(&format!(
        "Installed by provenance {}",
        env!("CARGO_PKG_VERSION")
    )));
    assert!(canonical_contents.contains("content hash fnv1a64:"));

    let link = dir.path().join(".claude/skills").join(skill);
    let link_metadata = std::fs::symlink_metadata(&link).unwrap();
    assert!(link_metadata.file_type().is_symlink());
    assert_eq!(
        std::fs::read_link(&link).unwrap(),
        PathBuf::from("../../.agents/skills/provenance-fork-tournament")
    );
    assert_eq!(
        std::fs::read_to_string(link.join("SKILL.md")).unwrap(),
        canonical_contents
    );
}

#[test]
fn fork_tournament_skill_documents_the_assertable_winner_lifecycle() {
    let skill = include_str!("../skills/provenance-fork-tournament/SKILL.md");

    assert!(skill.contains("--supporting-claim-id claim_<question>_<slot>"));
    assert!(skill.contains(r#""proposal_id":"prop_<question>_<slot>""#));
    assert!(skill.contains("provenance proposals assert --scope <scope>"));
    assert!(skill.contains("--id assertion_<question>_<winner_slot>"));
    assert!(skill.contains("--resolve-human-gate"));
    assert!(skill.contains(r#""decision_key":"pick_<question>_winner""#));
    assert!(skill.contains("--decision-key pick_<question>_winner"));
}

#[test]
fn skills_install_copy_flag_copies_claude_skills_instead_of_symlinking() {
    let dir = tempfile::tempdir().unwrap();
    let skill = "provenance-swarm-backtrace";

    install(dir.path(), &["--copy"])
        .success()
        .stdout(predicate::str::contains(r#""link_mode": "copy""#));

    let canonical = dir
        .path()
        .join(".agents/skills")
        .join(skill)
        .join("SKILL.md");
    let copied = dir
        .path()
        .join(".claude/skills")
        .join(skill)
        .join("SKILL.md");
    assert!(canonical.exists());
    assert!(copied.exists());
    assert!(!std::fs::symlink_metadata(copied.parent().unwrap())
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        std::fs::read_to_string(copied).unwrap(),
        std::fs::read_to_string(canonical).unwrap()
    );
}

#[test]
#[cfg(unix)]
#[verifies("rule_install_never_clobbers", examples)]
fn skills_install_copy_replaces_own_symlink_but_foreign_symlink_requires_force() {
    let dir = tempfile::tempdir().unwrap();
    let skill = "provenance-shaping";
    let link = dir.path().join(".claude/skills").join(skill);

    // Default install, then --copy: our own canonical symlink is replaced
    // with a real directory without needing --force.
    install(dir.path(), &[]).success();
    install(dir.path(), &["--copy"]).success();
    let metadata = std::fs::symlink_metadata(&link).unwrap();
    assert!(metadata.is_dir());
    assert!(!metadata.file_type().is_symlink());

    // A foreign symlink is not silently destroyed.
    std::fs::remove_dir_all(&link).unwrap();
    std::os::unix::fs::symlink("../../elsewhere", &link).unwrap();
    install(dir.path(), &["--copy"])
        .failure()
        .stderr(predicate::str::contains("rerun with --force"));
    assert_eq!(
        std::fs::read_link(&link).unwrap(),
        PathBuf::from("../../elsewhere")
    );

    install(dir.path(), &["--copy", "--force"]).success();
    assert!(std::fs::symlink_metadata(&link).unwrap().is_dir());
}

/// A plain file where a skill directory belongs: the symlink path and the
/// copy path both refuse it, and `--force` is what gets past it. The user's
/// file survives every refusal.
#[test]
#[verifies("rule_install_never_clobbers", examples)]
fn skills_install_refuses_a_file_where_a_skill_directory_belongs() {
    let dir = tempfile::tempdir().unwrap();
    let occupied = dir.path().join(".claude/skills/provenance-fork-tournament");
    std::fs::create_dir_all(occupied.parent().unwrap()).unwrap();
    std::fs::write(&occupied, "not a skill directory\n").unwrap();

    for arguments in [vec![], vec!["--copy"]] {
        install(dir.path(), &arguments)
            .failure()
            .stderr(predicate::str::contains(
                "exists and is not a skill directory; rerun with --force to overwrite",
            ));
        assert_eq!(
            std::fs::read_to_string(&occupied).unwrap(),
            "not a skill directory\n"
        );
    }

    install(dir.path(), &["--force"]).success();
    assert!(std::fs::metadata(&occupied).unwrap().is_dir());
    assert!(occupied.join("SKILL.md").exists());
}

/// The run-level `status` is read out of the parsed report rather than
/// matched anywhere in the output, so a per-file entry carrying the same word
/// cannot stand in for it.
#[test]
#[verifies("rule_install_never_clobbers", examples)]
#[verifies("rule_install_run_status", examples)]
fn skills_install_is_idempotent_and_requires_force_for_canonical_drift() {
    let dir = tempfile::tempdir().unwrap();
    let installed = dir
        .path()
        .join(".agents/skills/provenance-fork-tournament/SKILL.md");

    assert_eq!(install_status(dir.path(), &[]), "installed");
    assert_eq!(install_status(dir.path(), &[]), "unchanged");

    std::fs::write(&installed, "local edit\n").unwrap();
    install(dir.path(), &[])
        .failure()
        .stderr(predicate::str::contains("exists and differs"));

    assert_eq!(install_status(dir.path(), &["--force"]), "updated");
}

#[test]
fn skills_install_global_uses_home_agents_and_claude_skill_dirs() {
    let home = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(cwd.path())
        .env("HOME", home.path())
        .args([
            "skills", "install", "--global", "--copy", "--format", "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""global": true"#))
        .stdout(predicate::str::contains(r#""link_mode": "copy""#));

    assert!(home
        .path()
        .join(".agents/skills/provenance-grounded-writing/SKILL.md")
        .exists());
    assert!(home
        .path()
        .join(".claude/skills/provenance-grounded-writing/SKILL.md")
        .exists());
    assert!(!cwd
        .path()
        .join(".agents/skills/provenance-grounded-writing/SKILL.md")
        .exists());
}

#[test]
fn init_does_not_write_an_agents_md_skills_section() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");

    init_repo(&repo);

    let agents = repo.join("AGENTS.md");
    if agents.exists() {
        let contents = std::fs::read_to_string(agents).unwrap();
        assert!(!contents.contains("<!-- BEGIN PROVENANCE SKILLS -->"));
    }
}

#[test]
fn prime_reports_skill_install_status_and_install_command() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    init_repo(&repo);
    std::fs::remove_file(repo.join(".agents/skills/provenance-shaping/SKILL.md")).unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "prime",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""skills""#))
        .stdout(predicate::str::contains(r#""installed": false"#))
        .stdout(predicate::str::contains("provenance skills install"));

    install(&repo, &["--copy"]).success();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "prime",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""installed": true"#));
}

#[test]
fn install_status_uses_canonical_agents_skill_files_as_source_of_truth() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    init_repo(&repo);

    install(&repo, &["--copy"]).success();
    std::fs::remove_file(repo.join(".agents/skills/provenance-fork-tournament/SKILL.md")).unwrap();
    assert!(repo
        .join(".claude/skills/provenance-fork-tournament/SKILL.md")
        .exists());

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "prime",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""installed": false"#))
        .stdout(predicate::str::contains("provenance-fork-tournament"));
}

#[test]
fn shaping_and_ideation_commands_emit_suppressible_skill_install_hint() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    init_repo(&repo);
    std::fs::remove_file(repo.join(".agents/skills/provenance-shaping/SKILL.md")).unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "questions",
            "list",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "hint: provenance skills are not installed; run `provenance skills install`",
        ));

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "proposals",
            "list",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "hint: provenance skills are not installed; run `provenance skills install`",
        ));

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "--quiet",
            "questions",
            "list",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

/// Runs `provenance skills install` in `dir`, always asking for the JSON
/// report so a caller can read the run out of it.
fn install(dir: &std::path::Path, arguments: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(dir)
        .args(["skills", "install"])
        .args(arguments)
        .args(["--format", "json"])
        .assert()
}

/// The run-level `status` field of a successful install's JSON report.
fn install_status(dir: &std::path::Path, arguments: &[&str]) -> String {
    let output = install(dir, arguments)
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    report["status"].as_str().unwrap().to_string()
}

fn init_repo(repo: &std::path::Path) {
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
        ])
        .assert()
        .success();
}

fn workspace_skill_files() -> std::collections::BTreeMap<String, String> {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let mut skills = std::collections::BTreeMap::new();
    for entry in std::fs::read_dir(workspace.join("skills")).unwrap() {
        let entry = entry.unwrap();
        if !entry.file_type().unwrap().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let contents = std::fs::read_to_string(entry.path().join("SKILL.md")).unwrap();
        skills.insert(name, contents);
    }
    skills
}
