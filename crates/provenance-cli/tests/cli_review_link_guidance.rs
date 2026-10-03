use assert_cmd::Command;
use provenance_macros::verifies;

#[test]
#[verifies("rule_agent_review_request_includes_link", examples)]
fn installed_guidance_gives_people_review_links_instead_of_record_ids() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
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

    let agents = std::fs::read_to_string(repo.join("AGENTS.md")).unwrap();
    assert!(agents.contains("provenance review --repo"));
    assert!(agents.contains("get --review-link --format json"));
    assert!(agents.contains("Never give the person a record ID"));

    for skill in [
        "provenance-fork-tournament",
        "provenance-grounded-writing",
        "provenance-shaping",
        "provenance-swarm-backtrace",
    ] {
        let text =
            std::fs::read_to_string(repo.join(".agents/skills").join(skill).join("SKILL.md"))
                .unwrap();
        assert!(text.contains("provenance review --repo"), "{skill}");
        assert!(text.contains("get --review-link --format json"), "{skill}");
        assert!(
            text.contains("Never give the person a record ID"),
            "{skill}"
        );
    }
}
