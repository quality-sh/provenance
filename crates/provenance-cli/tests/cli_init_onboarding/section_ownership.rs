use super::*;

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
        onboarding_support::without_review_guidance(read_agents(&repo)),
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

    assert_eq!(
        onboarding_support::without_review_guidance(read_agents(&repo)),
        format!("{existing}\n{INSTRUCTIONS}\n")
    );
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

    assert_eq!(
        onboarding_support::without_review_guidance(read_agents(&repo)),
        format!("{existing}\n{INSTRUCTIONS}\n")
    );
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

    assert_eq!(
        onboarding_support::without_review_guidance(read_agents(&repo)),
        format!("{INSTRUCTIONS}\n\n{trailing}")
    );
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

    assert_eq!(
        onboarding_support::without_review_guidance(read_agents(&repo)),
        format!("{existing}\n{INSTRUCTIONS}\n")
    );
}
