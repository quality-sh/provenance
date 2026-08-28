use assert_cmd::Command;
use predicates::prelude::*;
use provenance_macros::verifies;
use std::path::Path;

const INSTRUCTIONS: &str = r#"## Provenance

Requirements live in a Provenance graph. Plan changes with the graph and update
it in the same change.

- Use the `provenance-grounded-writing` skill before you write or change a
  Requirement or Rule statement.
- Before a graph write, send `{"statement":"<statement>"}` to
  `provenance sdk check-statement --format json`. A clean report covers only the
  ASD-STE100 Issue 9 checks that Provenance implements. It does not prove full
  conformance.
- Plan: `provenance prime --quiet`
- New obligation: `provenance rules create --scope default --id rule_<slug> --requirement-id <req> --statement "<testable clause>"`
- Annotate implementation with `rule`, tests with `verifies`. Annotations move
  with code.
- To change a Requirement, Rule, or past decision, create a Proposal. A human decides each
  Proposal.
- Write graph state only through the Provenance CLI or SDK. Do not edit
  `.provenance/state` directly.
- Pre-commit: `provenance check --quiet` and
  `provenance coverage scan --path . --scope default --validate-rules`.
  Commit graph updates with the code.
- ASD owns ASD-STE100. STEMG maintains it. Use the official Issue 9 request page:
  https://www.asd-ste100.org/STE_downloads.html#article02-2l. Provenance names
  only its implemented checks and makes no compliance or endorsement claim."#;

#[test]
#[verifies("rule_init_installs_bundled_skills", examples)]
#[verifies("rule_init_owns_agents_provenance_section", examples)]
#[verifies("rule_init_native_command", examples)]
#[verifies("rule_init_grounded_writing_guidance", examples)]
#[verifies("rule_init_statement_preflight_guidance", examples)]
#[verifies("rule_init_statement_claim_limit", examples)]
#[verifies("rule_init_canonical_write_path", examples)]
fn init_installs_bundled_skills_and_ratified_instructions() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");

    init(&repo).success();

    for skill in [
        "provenance-fork-tournament",
        "provenance-grounded-writing",
        "provenance-shaping",
        "provenance-swarm-backtrace",
    ] {
        assert!(repo
            .join(".agents/skills")
            .join(skill)
            .join("SKILL.md")
            .exists());
        assert!(repo.join(".claude/skills").join(skill).exists());
    }
    assert_eq!(read_agents(&repo), format!("{INSTRUCTIONS}\n"));
    assert_eq!(
        std::fs::read_to_string(repo.join(".gitignore")).unwrap(),
        ".provenance/cache/\n"
    );
}

#[test]
#[verifies("rule_init_installs_bundled_skills", examples)]
#[verifies("rule_init_owns_agents_provenance_section", examples)]
fn init_onboarding_is_idempotent() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    init(&repo).success();
    let agents = read_agents(&repo);
    let skill = std::fs::read(repo.join(".agents/skills/provenance-shaping/SKILL.md")).unwrap();

    init(&repo).success();

    assert_eq!(read_agents(&repo), agents);
    assert_eq!(
        std::fs::read(repo.join(".agents/skills/provenance-shaping/SKILL.md")).unwrap(),
        skill
    );
}

#[test]
#[verifies("rule_init_typescript_local_command", examples)]
fn typescript_init_writes_a_package_local_command() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");

    init_with(
        &repo,
        &[
            "--invocation-channel",
            "typescript",
            "--package-manager",
            "npm",
        ],
    )
    .success();

    let agents = read_agents(&repo);
    assert!(agents.contains("`npx --no provenance prime --quiet`"));
    assert!(agents.contains("`npx --no provenance sdk check-statement --format json`"));
    assert!(!agents.contains("`provenance prime --quiet`"));
}

#[cfg(unix)]
#[test]
#[verifies("rule_init_managed_paths_stay_in_repository", examples)]
fn init_refuses_managed_skill_paths_with_symlinked_ancestors() {
    use std::os::unix::fs::symlink;

    for managed_directory in [".agents", ".claude"] {
        let temporary = tempfile::tempdir().unwrap();
        let repo = temporary.path().join("repo");
        let outside = temporary.path().join("outside");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        symlink(&outside, repo.join(managed_directory)).unwrap();

        init(&repo)
            .failure()
            .stderr(predicate::str::contains("symlink component"));

        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
        assert!(!repo.join(".provenance").exists());
    }
}

#[test]
#[verifies("rule_init_installs_bundled_skills", examples)]
#[verifies("rule_init_upgrades_hash_owned_skills", examples)]
fn init_upgrades_an_unedited_skill_from_a_prior_provenance_version() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    init(&repo).success();
    let skill = repo.join(".agents/skills/provenance-shaping/SKILL.md");
    let current = std::fs::read_to_string(&skill).unwrap();
    write_as_prior_version(&skill, &current);

    init(&repo).success();

    assert_eq!(std::fs::read_to_string(skill).unwrap(), current);
}

#[test]
#[verifies("rule_init_installs_bundled_skills", examples)]
#[verifies("rule_init_upgrades_hash_owned_skills", examples)]
fn init_refuses_to_replace_an_edited_skill_from_a_prior_provenance_version() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    init(&repo).success();
    let skill = repo.join(".agents/skills/provenance-shaping/SKILL.md");
    let current = std::fs::read_to_string(&skill).unwrap();
    write_as_prior_version(&skill, &current);
    let mut edited = std::fs::read_to_string(&skill).unwrap();
    edited.push_str("\nUser edit.\n");
    std::fs::write(&skill, &edited).unwrap();

    init(&repo)
        .failure()
        .stderr(predicate::str::contains("exists and differs"))
        .stderr(predicate::str::contains("rerun with --force"));

    assert_eq!(std::fs::read_to_string(skill).unwrap(), edited);
}

#[test]
#[verifies("rule_init_upgrades_hash_owned_skills", examples)]
#[verifies("rule_init_plan_rejection_preserves_targets", examples)]
fn a_skill_conflict_leaves_a_new_repository_without_partial_onboarding() {
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path().join("source");
    let repo = temporary.path().join("repo");
    init(&source).success();
    let source_skill = source.join(".agents/skills/provenance-shaping/SKILL.md");
    let target_skill = repo.join(".agents/skills/provenance-shaping/SKILL.md");
    std::fs::create_dir_all(target_skill.parent().unwrap()).unwrap();
    let current = std::fs::read_to_string(source_skill).unwrap();
    write_as_prior_version(&target_skill, &current);
    let mut edited = std::fs::read_to_string(&target_skill).unwrap();
    edited.push_str("\nUser edit.\n");
    std::fs::write(&target_skill, &edited).unwrap();

    init(&repo)
        .failure()
        .stderr(predicate::str::contains("exists and differs"));

    assert_eq!(std::fs::read_to_string(target_skill).unwrap(), edited);
    assert!(!repo.join(".provenance").exists());
    assert!(!repo.join(".claude").exists());
    assert!(!repo.join("AGENTS.md").exists());
    assert!(!repo.join(".gitignore").exists());
}

#[test]
#[verifies("rule_init_plans_all_project_writes", examples)]
#[verifies("rule_init_plan_rejection_preserves_targets", examples)]
fn a_late_claude_conflict_leaves_every_existing_file_unchanged() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    std::fs::create_dir_all(repo.join(".claude/skills")).unwrap();
    std::fs::create_dir_all(repo.join(".provenance/state")).unwrap();
    std::fs::write(repo.join("keep.txt"), "keep\n").unwrap();
    std::fs::write(repo.join("AGENTS.md"), "# Existing\n").unwrap();
    std::fs::write(repo.join(".gitignore"), "target/\n").unwrap();
    let conflict = repo.join(".claude/skills/provenance-swarm-backtrace");
    std::fs::write(&conflict, "user-owned\n").unwrap();

    init(&repo)
        .failure()
        .stderr(predicate::str::contains("rerun with --force"));

    assert_eq!(
        std::fs::read_to_string(repo.join("keep.txt")).unwrap(),
        "keep\n"
    );
    assert_eq!(
        std::fs::read_to_string(repo.join("AGENTS.md")).unwrap(),
        "# Existing\n"
    );
    assert_eq!(
        std::fs::read_to_string(repo.join(".gitignore")).unwrap(),
        "target/\n"
    );
    assert_eq!(std::fs::read_to_string(conflict).unwrap(), "user-owned\n");
    assert!(!repo.join(".provenance/state/manifest.json").exists());
    assert!(!repo.join(".agents").exists());
}

#[test]
#[verifies("rule_init_plan_rejection_preserves_targets", examples)]
#[verifies("rule_init_validates_planned_repository", examples)]
fn invalid_existing_graph_is_rejected_before_onboarding_writes() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    let graph = repo.join(".provenance/state/scopes/default/requirements/req.jsonl");
    std::fs::create_dir_all(graph.parent().unwrap()).unwrap();
    std::fs::write(&graph, "not json\n").unwrap();

    init(&repo).failure();

    assert_eq!(std::fs::read_to_string(graph).unwrap(), "not json\n");
    assert!(!repo.join(".provenance/state/manifest.json").exists());
    assert!(!repo.join(".agents").exists());
    assert!(!repo.join(".claude").exists());
    assert!(!repo.join("AGENTS.md").exists());
    assert!(!repo.join(".gitignore").exists());
}

#[test]
fn invalid_agents_text_is_rejected_before_any_init_write() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let invalid = b"instructions\xff";
    std::fs::write(repo.join("AGENTS.md"), invalid).unwrap();

    init(&repo)
        .failure()
        .stderr(predicate::str::contains("UTF-8"));

    assert_eq!(std::fs::read(repo.join("AGENTS.md")).unwrap(), invalid);
    assert!(!repo.join(".provenance").exists());
    assert!(!repo.join(".agents").exists());
    assert!(!repo.join(".claude").exists());
    assert!(!repo.join(".gitignore").exists());
}

#[test]
fn invalid_gitignore_text_is_rejected_before_any_init_write() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let invalid = b"target/\n\xff";
    std::fs::write(repo.join(".gitignore"), invalid).unwrap();

    init(&repo)
        .failure()
        .stderr(predicate::str::contains("UTF-8"));

    assert_eq!(std::fs::read(repo.join(".gitignore")).unwrap(), invalid);
    assert!(!repo.join(".provenance").exists());
    assert!(!repo.join(".agents").exists());
    assert!(!repo.join(".claude").exists());
    assert!(!repo.join("AGENTS.md").exists());
}

#[test]
fn existing_manifest_is_preserved_when_a_late_skill_conflicts() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    init(&repo).success();
    let manifest = repo.join(".provenance/state/manifest.json");
    let before = std::fs::read(&manifest).unwrap();
    let conflict = repo.join(".claude/skills/provenance-swarm-backtrace");
    remove_skill_entry(&conflict);
    std::fs::write(&conflict, "user-owned\n").unwrap();

    init(&repo)
        .failure()
        .stderr(predicate::str::contains("rerun with --force"));

    assert_eq!(std::fs::read(manifest).unwrap(), before);
    assert_eq!(std::fs::read_to_string(conflict).unwrap(), "user-owned\n");
}

#[test]
#[verifies("rule_init_plan_rejection_preserves_targets", examples)]
#[verifies("rule_init_validates_planned_repository", examples)]
fn planned_validation_failure_preserves_the_original_repository() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    let original_scope = repo.join(".provenance/state/scopes/unexpected");
    std::fs::create_dir_all(&original_scope).unwrap();

    init(&repo)
        .failure()
        .stderr(predicate::str::contains("scope directory unexpected"));

    assert!(original_scope.is_dir());
    assert!(!repo.join(".provenance/manifest.json").exists());
    assert!(!repo.join(".provenance/state/edges").exists());
    assert!(repo
        .join(".provenance/cache/locks/repository.publication.lock")
        .is_file());
    assert!(!repo.join(".agents").exists());
    assert!(!repo.join(".claude").exists());
    assert!(!repo.join("AGENTS.md").exists());
    assert!(!repo.join(".gitignore").exists());
}

#[test]
#[verifies("rule_init_owns_agents_provenance_section", examples)]
fn init_updates_the_exact_heading_and_preserves_other_content() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(
        repo.join("AGENTS.md"),
        "# Local instructions\n\nKeep this.\n\n## Provenance\n\nOld text.\n\n## Build\n\nKeep this too.\n",
    )
    .unwrap();

    init(&repo).success();

    assert_eq!(
        read_agents(&repo),
        format!(
            "# Local instructions\n\nKeep this.\n\n{INSTRUCTIONS}\n\n## Build\n\nKeep this too.\n"
        )
    );
}

#[test]
#[verifies("rule_init_owns_agents_provenance_section", examples)]
fn init_leaves_a_renamed_heading_alone_and_adds_the_owned_section() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let existing = "# Local instructions\n\n## Project Provenance\n\nUser-owned text.\n";
    std::fs::write(repo.join("AGENTS.md"), existing).unwrap();

    init(&repo).success();

    assert_eq!(read_agents(&repo), format!("{existing}\n{INSTRUCTIONS}\n"));
}

#[test]
#[verifies("rule_init_owns_agents_provenance_section", examples)]
fn init_ignores_headings_inside_fenced_examples() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let existing = "# Local instructions\n\n```md\n## Provenance\nExample only.\n\n## Build\nStill an example.\n```\n";
    std::fs::write(repo.join("AGENTS.md"), existing).unwrap();

    init(&repo).success();

    assert_eq!(read_agents(&repo), format!("{existing}\n{INSTRUCTIONS}\n"));
}

#[test]
#[verifies("rule_init_owns_agents_provenance_section", examples)]
fn init_preserves_a_following_setext_section() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let trailing = "Build\n-----\n\nUser-owned text.\n";
    std::fs::write(
        repo.join("AGENTS.md"),
        format!("## Provenance\n\nOld text.\n\n{trailing}"),
    )
    .unwrap();

    init(&repo).success();

    assert_eq!(read_agents(&repo), format!("{INSTRUCTIONS}\n\n{trailing}"));
}

#[test]
#[verifies("rule_init_owns_agents_provenance_section", examples)]
fn init_does_not_claim_a_blockquoted_provenance_heading() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let existing = "> ## Provenance\n>\n> User-owned example.\n";
    std::fs::write(repo.join("AGENTS.md"), existing).unwrap();

    init(&repo).success();

    assert_eq!(read_agents(&repo), format!("{existing}\n{INSTRUCTIONS}\n"));
}

fn init(repo: &Path) -> assert_cmd::assert::Assert {
    init_with(repo, &[])
}

fn init_with(repo: &Path, extra: &[&str]) -> assert_cmd::assert::Assert {
    let mut command = Command::cargo_bin("provenance").unwrap();
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

fn read_agents(repo: &Path) -> String {
    std::fs::read_to_string(repo.join("AGENTS.md")).unwrap()
}

fn write_as_prior_version(path: &Path, current: &str) {
    let current_stamp = format!("Installed by provenance {}", env!("CARGO_PKG_VERSION"));
    let prior = current.replacen(&current_stamp, "Installed by provenance 0.2.1", 1);
    assert_ne!(prior, current);
    std::fs::write(path, prior).unwrap();
}

fn remove_skill_entry(path: &Path) {
    let metadata = std::fs::symlink_metadata(path).unwrap();
    if metadata.file_type().is_symlink() {
        remove_skill_symlink(path);
    } else if metadata.is_file() {
        std::fs::remove_file(path).unwrap();
    } else {
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[cfg(unix)]
fn remove_skill_symlink(path: &Path) {
    std::fs::remove_file(path).unwrap();
}

#[cfg(windows)]
fn remove_skill_symlink(path: &Path) {
    std::fs::remove_dir(path).unwrap();
}
