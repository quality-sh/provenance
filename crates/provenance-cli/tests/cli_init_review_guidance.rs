//! The generated AGENTS.md tells an agent when to start the review host, how to hand the
//! person its link, what a description holds, and how to write graph state.

use assert_cmd::Command;
use provenance_macros::verifies;
use std::path::Path;

fn agents_after_init() -> String {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
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
    std::fs::read_to_string(Path::new(&repo).join("AGENTS.md")).unwrap()
}

fn normalized(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
#[verifies("rule_init_canonical_write_path", examples)]
fn init_guidance_requires_cli_or_sdk_writes_and_forbids_direct_state_edits() {
    let agents = agents_after_init();
    let prose = normalized(&agents);

    assert!(
        prose.contains("Write graph state only through the Provenance CLI or SDK."),
        "{agents}"
    );
    assert!(
        prose.contains("Do not edit `.provenance/state` directly."),
        "{agents}"
    );
}

#[test]
#[verifies("rule_init_review_host_guidance", examples)]
fn init_guidance_starts_the_host_early_and_hands_the_link_over_as_text() {
    let agents = agents_after_init();
    let prose = normalized(&agents);

    assert!(
        prose.contains("When a session starts to write records, start or reuse the review host:"),
        "{agents}"
    );
    assert!(
        agents.contains(
            "`provenance review --repo . --repository-id local --scope default --detach --no-open`"
        ),
        "{agents}"
    );
    assert!(
        prose.contains(
            "One host serves the repository and scope for all sessions. It keeps running after \
             the command returns, and later sessions reuse it, so leave it running."
        ),
        "{agents}"
    );
    assert!(
        prose.contains(
            "give the person `review_url` as plain text in chat, whether or not `opened` is true."
        ),
        "{agents}"
    );
    assert!(
        prose.contains("Opening the page is extra. Always give the link in chat too."),
        "{agents}"
    );
    assert!(
        prose.contains("A link works once and expires after 120 seconds."),
        "{agents}"
    );
    assert!(
        prose.contains(
            "The page is served on the machine that runs the host. If that is not the \
             person's machine, say so, and give the person this command to run on their own \
             machine: `provenance review --repo . --repository-id local --scope default \
             --detach`. It opens the page in their browser."
        ),
        "{agents}"
    );
    assert!(
        !prose.contains("Before you ask a person to review a record"),
        "{agents}"
    );
}

#[test]
#[verifies("rule_init_description_guidance", examples)]
fn init_guidance_says_what_a_description_holds() {
    let agents = agents_after_init();
    let prose = normalized(&agents);

    assert!(
        prose.contains(
            "A Requirement or Rule description says how the product behaves, in the present \
             tense: its states, what changes them, what the person sees, and one example."
        ),
        "{agents}"
    );
    assert!(
        prose.contains("History goes on the record's thread. Unknowns go in fog or a Question."),
        "{agents}"
    );
    assert!(
        prose.contains(
            "Before you hand off, read a Requirement and every record under it in order, as a \
             builder would: `provenance requirements <requirement-id> document get --scope \
             default --format json`."
        ),
        "{agents}"
    );
}
