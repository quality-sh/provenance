//! The shared guidance of a consumer names only the commands that the consumer shows.
//! These tests have no Rule binding: the Rule that states which commands the guidance
//! names waits for the approval of the owner.

use provenance_porcelain::action::Action;
use provenance_porcelain::guidance::{change_description, guidance, guidance_for};
use provenance_porcelain::registry::Command;

/// Phrases that use a command name as a noun: the trace view of get, and the claim of
/// a Topic. They do not name the command.
const NOUNS: [&str; 2] = ["trace view", "Topic claim"];

/// The action section of a guidance text, with the nouns removed.
fn action_section(text: &str) -> String {
    let (_, actions) = text
        .split_once("## Actions\n\n")
        .expect("an action section");
    NOUNS
        .iter()
        .fold(actions.to_owned(), |text, noun| text.replace(noun, ""))
}

/// Whether `text` names `name` as whole words, after each other command name that
/// holds `name` (as `dictionary import` holds `import`) is removed.
fn names(text: &str, name: &str) -> bool {
    let text = Command::ALL
        .iter()
        .map(|command| command.declaration().name)
        .filter(|other| *other != name && other.contains(name))
        .fold(text.to_owned(), |text, other| text.replace(other, ""));
    let word = |c: char| c.is_alphanumeric() || c == '-' || c == '_';
    text.match_indices(name).any(|(at, _)| {
        let before = text[..at].chars().next_back().is_none_or(|c| !word(c));
        let after = text[at + name.len()..]
            .chars()
            .next()
            .is_none_or(|c| !word(c));
        before && after
    })
}

#[test]
fn the_guidance_names_no_command_that_the_consumer_does_not_show() {
    for hidden in Command::ALL.iter().copied() {
        let text = guidance_for(|command| command != hidden, true).guidance;
        let name = hidden.declaration().name;
        assert!(
            !names(&action_section(&text), name),
            "the guidance without {name} names it:\n{text}"
        );
    }
    let text = guidance_for(|_| true, false).guidance;
    assert!(
        !names(&action_section(&text), "notify"),
        "the guidance without notify names it:\n{text}"
    );
}

#[test]
fn the_change_line_with_each_write_is_the_change_declaration_description() {
    let description = Command::Change.declaration().description;
    assert_eq!(
        change_description(|_| true, true).as_deref(),
        Some(description)
    );
    assert!(guidance()
        .guidance
        .contains(&format!("\n- change: {description}\n")));
    assert_eq!(
        change_description(|command| command == Command::Change, false),
        None
    );
}

#[test]
fn the_create_sentence_names_existing_record_actions_only_when_one_is_shown() {
    let existing = |command: Command| {
        matches!(command, Command::Action(action)
            if Action::RECORD.contains(&action) && action != Action::Create)
    };
    let text = guidance_for(|command| !existing(command), true).guidance;
    assert!(
        text.contains("\nCreate names an explicit record type.\n"),
        "{text}"
    );
    assert!(
        !text.contains("actions on an existing record infer its type"),
        "{text}"
    );
    assert!(guidance().guidance.contains(
        "\nUse search to find IDs, then get to read the selected record and its related views. \
         Create names an explicit record type; actions on an existing record infer its type.\n"
    ));
}

/// No MCP host lists trace without get, because both tools need the same read grant.
/// The shared guidance does not know its consumer, so this porcelain test is the
/// evidence: the trace line names get and goes with it.
#[test]
fn guidance_with_trace_and_without_get_gives_no_trace_line() {
    let without_get = guidance_for(|command| command != Command::Get, true).guidance;
    let without_get_and_trace = guidance_for(
        |command| !matches!(command, Command::Get | Command::Trace),
        true,
    )
    .guidance;
    assert_eq!(without_get, without_get_and_trace);
    assert!(guidance().guidance.contains("\n- trace: "));
}
