//! Pins the shaping guidance that makes a requirement tree read as a spec and puts the
//! review page in front of the person from the first reply.
//!
//! The sentences are read out of the binary, which is the copy an installed agent gets.

use assert_cmd::Command;
use provenance_macros::verifies;

fn skill(name: &str) -> String {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args(["skills", "show", name])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(output).unwrap()
}

fn normalized(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Both skills say what a description holds and where history and unknowns go instead.
#[test]
#[verifies("rule_shaping_skill_description_is_behaviour", examples)]
fn shaping_guidance_makes_descriptions_state_behaviour_not_history() {
    let shaping = skill("provenance-shaping");
    let prose = normalized(&shaping);
    assert!(shaping.contains("## The tree is the spec"), "{shaping}");
    assert!(
        prose.contains(
            "how the product behaves, in the present tense: its states, what moves it between \
             them, what the person sees, and one example or edge case."
        ),
        "{shaping}"
    );
    assert!(
        prose.contains(
            "The person's words, the brief's words, and what changed go here, never in a \
             description."
        ),
        "{shaping}"
    );
    assert!(
        prose.contains("A description does not list open items or say what remains to be shaped."),
        "{shaping}"
    );
    assert!(
        prose.contains("read the tree as a builder would"),
        "{shaping}"
    );

    let writing = skill("provenance-grounded-writing");
    let prose = normalized(&writing);
    assert!(
        prose.contains("A description says how the product behaves, in the present tense"),
        "{writing}"
    );
    assert!(
        prose.contains(
            "It does not retell the brief, does not say who confirmed what, and does not list \
             what remains to be shaped."
        ),
        "{writing}"
    );
    assert!(prose.contains("Description as minutes — bad:"), "{writing}");
    assert!(prose.contains("Description as spec — good:"), "{writing}");
    assert!(!prose.contains("where being fuller is fine"), "{writing}");
}

/// A Question closes only on a complete answer.
#[test]
#[verifies("rule_shaping_skill_answers_cover_the_question", examples)]
fn shaping_guidance_answers_a_question_only_when_the_answer_covers_it() {
    let shaping = skill("provenance-shaping");
    let prose = normalized(&shaping);
    assert!(
        prose.contains("A Question is answered only when the answer covers all of it."),
        "{shaping}"
    );
    assert!(
        prose.contains(
            "If an answer settles part of a Question, land that part, leave the Question open, \
             and narrow its text to what remains."
        ),
        "{shaping}"
    );
    assert!(
        prose.contains("a Question marked answered is covered completely by its answer;"),
        "{shaping}"
    );
}

/// The review host starts with the session, and the link reaches the person as text.
#[test]
#[verifies("rule_shaping_skill_gives_review_link_first", examples)]
fn shaping_guidance_puts_the_review_link_in_the_first_reply() {
    let shaping = skill("provenance-shaping");
    let prose = normalized(&shaping);
    assert!(
        shaping.contains(
            "provenance review --repo . --repository-id local --scope <scope> --detach --no-open"
        ),
        "{shaping}"
    );
    assert!(
        prose.contains(
            "Start or reuse the review host, and give the person its link in your first reply:"
        ),
        "{shaping}"
    );
    assert!(
        prose.contains("Paste `review_url` from its output into the chat as plain text."),
        "{shaping}"
    );
    assert!(
        prose.contains("the pasted link is the route that always works."),
        "{shaping}"
    );
    assert!(
        prose.contains("The page is served on the machine that runs the host."),
        "{shaping}"
    );
    assert!(
        prose.contains("paste the anchor's review link again."),
        "{shaping}"
    );
}
