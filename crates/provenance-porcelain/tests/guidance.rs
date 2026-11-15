//! The shared guidance of a consumer names only the commands that the consumer shows.
//! These tests have no Rule binding: the Rule that states which commands the guidance
//! names waits for the approval of the owner.

use provenance_porcelain::action::Action;
use provenance_porcelain::guidance::{change_description, guidance, guidance_for};
use provenance_porcelain::registry::Command;

/// Phrases that do not name a command, each with the start of the one line that holds
/// it: the trace view of get, the claim of a Topic, and the HTTP method of an API route.
const NOT_COMMANDS: [(&str, &str); 3] = [
    ("- get: ", "trace view"),
    ("- release: ", "Topic claim"),
    ("- api: ", "GET /rules"),
];

/// The record kind that has the name of a command. The guidance writes the record kind
/// with a capital letter.
const RECORD_KIND: &str = "Discussion";

/// The full guidance text in lower case, without the phrases that do not name a
/// command and without the record kind.
fn command_text(text: &str) -> String {
    text.lines()
        .map(|line| {
            NOT_COMMANDS
                .iter()
                .filter(|(start, _)| line.starts_with(start))
                .fold(line.to_owned(), |line, (_, phrase)| {
                    line.replace(phrase, "")
                })
                .replace(RECORD_KIND, "")
                .to_lowercase()
        })
        .collect::<Vec<_>>()
        .join("\n")
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
            !names(&command_text(&text), name),
            "the guidance without {name} names it:\n{text}"
        );
    }
    let text = guidance_for(|_| true, false).guidance;
    assert!(
        !names(&command_text(&text), "notify"),
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
        text.contains(" Create names an explicit record type.\n"),
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

/// The `edit.etag` sentence names get and update, so a consumer gets it only when it shows
/// the two commands.
#[test]
fn the_etag_sentence_needs_get_and_update() {
    let sentence = "Before an update, read the record with get. Give its `edit.etag` unchanged \
                    to the update as the if-match value.\n";
    let update = Command::Action(Action::Update);
    let without_update = guidance_for(|command| command != update, true).guidance;
    assert!(!without_update.contains(sentence), "{without_update}");
    let without_get = guidance_for(|command| command != Command::Get, true).guidance;
    assert!(!without_get.contains(sentence), "{without_get}");
    let get_and_update =
        guidance_for(|command| command == Command::Get || command == update, true).guidance;
    assert_eq!(
        get_and_update.matches(sentence).count(),
        1,
        "{get_and_update}"
    );
}
