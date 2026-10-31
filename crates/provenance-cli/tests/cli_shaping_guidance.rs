//! Pins the shaping guidance that makes a requirement tree read as a spec and puts the
//! requirements view in front of the person before the first question.
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
             them, what the person sees, and one example."
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
        prose.contains(
            "read the tree as a builder would. Read the anchor's requirement document with \
             `provenance requirements <anchor_requirement_id> document get"
        ),
        "{shaping}"
    );
    assert!(
        prose.contains("and stays in review with `--status review`."),
        "{shaping}"
    );
    assert!(
        prose.contains("--severity high \\ --status review \\ --format json"),
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
    assert!(
        prose.contains("what the person sees, and one example. Write it for a builder"),
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
        prose.contains("records and answers meet \"The tree is the spec\";"),
        "{shaping}"
    );
}

/// The requirements view opens before the first question, in the place where the person
/// can see it, with the anchor inside the tree of its parent.
#[test]
#[verifies("rule_shaping_skill_gives_review_link_first", examples)]
fn shaping_guidance_opens_the_requirements_view_before_the_first_question() {
    let shaping = skill("provenance-shaping");
    let prose = normalized(&shaping);
    for text in [
        "**Open the requirements view first.** Do this before the first question and before \
         the first graph write.",
        "The view shows the anchor inside the tree of its parent, not as a root. Open the \
         page of the anchor's parent Requirement",
        "Never open the page with the anchor as its root.",
        "**A desktop app with a browser pane** (Claude Code desktop, Codex desktop)",
        "open `review_url` in the browser pane",
        "**Claude Desktop or ChatGPT desktop** (MCP tools, no browser pane): open the MCP \
         document view.",
        "**A plain CLI session**",
        "Do not add `--no-open` in a CLI session.",
        "Tell the person to keep the page open and to watch the records land there after \
         each answer.",
        "**Exception: an anchor with no parent.** Create that anchor first, then open its own \
         page. This is the only graph write before the view opens.",
        "In Chart mode, open the view of the parent first, then create the anchor under that \
         parent.",
    ] {
        assert!(prose.contains(text), "missing {text}\n{shaping}");
    }
    let cli = "provenance review --repo . --repository-id local --scope <scope> --detach\n";
    assert!(shaping.contains(cli), "{shaping}");
    let link = "provenance <parent_requirement_id> --scope <scope> --review-link --format json";
    assert!(shaping.contains(link), "{shaping}");
    assert!(!prose
        .contains("Opening the page in a browser or in the app that shows your chat is extra."));
    assert!(
        !prose.contains("paste the anchor's review link again."),
        "{shaping}"
    );
}

/// A decision becomes a Resolution that spawns a child Requirement, and its Rules go under
/// that child with the Resolution as producer, never directly under the anchor.
#[test]
#[verifies("rule_shaping_skill_tree_branches_by_decision", examples)]
fn shaping_guidance_puts_each_decision_on_its_own_branch() {
    let shaping = skill("provenance-shaping");
    let prose = normalized(&shaping);
    for text in [
        "A decision that settles a Question becomes a Resolution. The Resolution spawns a \
         child Requirement: `--refines <requirement_id> --spawned-by <resolution_id>`.",
        "The Rules of that decision go under the child Requirement, and each one names the \
         Resolution with `--resolution-id`. The CLI accepts this link while the Resolution \
         is proposed.",
        "every Resolution needs a spawned child Requirement and at least one Rule that names it.",
        "Never put a Rule directly under the anchor.",
        "Do not wait for approval to add the link:",
        "A person who accepts one option and rejects another has made a choice.",
        "do not skip the Resolution because no reason was given.",
    ] {
        assert!(prose.contains(text), "missing {text}\n{shaping}");
    }
    assert!(!prose.contains("Add `--resolution-id` only for an approved Resolution"));
    assert!(
        !prose.contains("Choice with no spawned record"),
        "{shaping}"
    );
}

/// The behaviour statement of the spawned child and the link from a Rule to its Resolution
/// are stated once, in "The shape of the tree", and the other places point to that section.
#[test]
#[verifies("rule_shaping_skill_tree_branches_by_decision", examples)]
fn shaping_guidance_states_each_branch_obligation_once() {
    let shaping = skill("provenance-shaping");
    let prose = normalized(&shaping);
    for (text, count) in [
        ("not that a choice was made", 1),
        ("not \"option X was selected\"", 0),
        ("names that Resolution at once", 0),
        ("each one names the Resolution with `--resolution-id`", 1),
        (
            "\"The shape of the tree\" gives what the spawned child Requirement states.",
            1,
        ),
        (
            "\"The shape of the tree\" gives its link to the Resolution.",
            1,
        ),
    ] {
        assert_eq!(prose.matches(text).count(), count, "{text}\n{shaping}");
    }
}

/// Each record that a session creates gets its own review submission.
#[test]
#[verifies("rule_shaping_skill_submits_every_record", examples)]
fn shaping_guidance_submits_every_created_record_for_review() {
    let shaping = skill("provenance-shaping");
    let prose = normalized(&shaping);
    for text in [
        "Submit every record that the session creates for review at once, so each one has its \
         own Approve control on the page.",
        "Rule, Resolution, Boundary and Source create do not. Submit each of these immediately \
         after you create it:",
        "provenance <record_id> submit --scope <scope> --actor <actor> \\",
        "each new record has a pending review submission (\"Every record goes to review\");",
    ] {
        assert!(prose.contains(text), "missing {text}\n{shaping}");
    }
}

/// An agent keeps the stored prerequisites and does not work a Question that is not ready.
#[test]
#[verifies("rule_shaping_skill_keeps_stored_prerequisites", examples)]
fn shaping_guidance_keeps_the_stored_question_order() {
    let shaping = skill("provenance-shaping");
    let prose = normalized(&shaping);
    for text in [
        "If one of them is not answered, the Question is not ready: do not claim it and do \
         not land its decision.",
        "Do not add a prerequisite to a Question that existed before your session, and do not \
         rewrite the text of such a Question.",
        "When the person answers an existing Question, answer that Question in the graph,",
    ] {
        assert!(prose.contains(text), "missing {text}\n{shaping}");
    }
}

/// The skill and the managed `AGENTS.md` lines tell the author to keep the
/// archive commit on main with a merge commit.
#[test]
fn archive_guidance_asks_for_a_merge_commit() {
    let merge = "Merge a pull request that archives records with a merge commit. A squash or \
                 rebase merge drops the commit that the archive names.";
    let shaping = skill("provenance-shaping");
    assert!(normalized(&shaping).contains(merge), "{shaping}");

    let directory = tempfile::tempdir().unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["init", "--path", directory.path().to_str().unwrap()])
        .args(["--scope", "default", "--path-prefix", "."])
        .assert()
        .success();
    let agents = std::fs::read_to_string(directory.path().join("AGENTS.md")).unwrap();
    assert!(normalized(&agents).contains(merge), "{agents}");
}
