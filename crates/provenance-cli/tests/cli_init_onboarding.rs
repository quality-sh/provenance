use assert_cmd::Command;
use provenance_macros::verifies;
use std::path::Path;

const INSTRUCTIONS: &str = r#"## Provenance

Requirements live in a Provenance graph. Plan changes with the graph and update
it in the same change.

- Plan: `provenance prime --quiet`
- New obligation: `provenance rules create --scope default --id rule_<slug> --requirement-id <req> --statement "<testable clause>"`
- Annotate implementation with `rule`, tests with `verifies`. Annotations move
  with code.
- To change a Requirement, Rule, or past decision, create a Proposal. A human decides each
  Proposal.
- Pre-commit: `provenance check --quiet` and
  `provenance coverage scan --path . --scope default --validate-rules`.
  Commit graph updates with the code."#;

#[test]
#[verifies("rule_init_installs_bundled_skills", examples)]
#[verifies("rule_init_owns_agents_provenance_section", examples)]
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
    let mut command = Command::cargo_bin("provenance").unwrap();
    command
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
}

fn read_agents(repo: &Path) -> String {
    std::fs::read_to_string(repo.join("AGENTS.md")).unwrap()
}
