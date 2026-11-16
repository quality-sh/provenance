//! The shared guidance of a consumer has one line for each command that the consumer
//! shows, with the name of the consumer, and names no command that the consumer does not
//! show. Its sentences name only shown commands.

use provenance_porcelain::action::Action;
use provenance_porcelain::guidance::{discussions_description, guidance_for, Shown};
use provenance_porcelain::registry::{
    Command, Declaration, Fields, GraphCommand, RepositoryCommand,
};

/// Phrases that do not name a command, each with the start of the one line that holds
/// it: the trace view of get, the claim of a Topic, the HTTP method of an API route, the
/// file that import reads, the checks of validate, and the verb of dictionary import.
const NOT_COMMANDS: [(&str, &str); 6] = [
    ("- get: ", "trace view"),
    ("- release: ", "Topic claim"),
    ("- api: ", "GET /rules"),
    ("- import: ", "export file"),
    ("- validate: ", "Check one"),
    ("- dictionary import: ", "Import the"),
];

/// The writes of the change command of the consumer in these tests, other than the
/// record actions and the Discussion writes.
const REPOSITORY_WRITES: [Command; 4] = [
    Command::Repository(RepositoryCommand::Materialize),
    Command::Repository(RepositoryCommand::Import),
    Command::Repository(RepositoryCommand::DictionaryImport),
    Command::Graph(GraphCommand::SwarmBacktraceLand),
];

/// The name that the consumer in these tests gives to the notify write. It is not the
/// name of a command.
const NOTIFY: &str = "watch";

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

/// Each command for which `shown` is true, in registry order, with its declaration name.
fn declared(shown: impl Fn(Command) -> bool) -> Vec<(Command, String)> {
    Command::ALL
        .iter()
        .copied()
        .filter(|command| shown(*command))
        .map(|command| (command, command.declaration().name.to_owned()))
        .collect()
}

/// Each registry command of `commands` as a command that the consumer shows.
fn registry(commands: Vec<(Command, String)>) -> Vec<(Shown, String)> {
    commands
        .into_iter()
        .map(|(command, name)| (Shown::Registry(command), name))
        .collect()
}

/// Whether the change command of the consumer selects `command`: an action that is not a
/// Discussion read, or a repository write.
fn is_write(command: Command) -> bool {
    let reads = [Action::Discussions, Action::Discussion].map(Command::Action);
    (matches!(command, Command::Action(_)) && !reads.contains(&command))
        || REPOSITORY_WRITES.contains(&command)
}

/// The guidance of a consumer that shows each command for which `shown` is true, with
/// the shown writes on its change command, and the notify write when `notify` is true.
fn guidance_with(shown: impl Fn(Command) -> bool, notify: bool) -> String {
    let commands = registry(declared(&shown));
    let writes = declared(|command| shown(command) && is_write(command));
    guidance_for(&commands, &writes, notify.then_some(NOTIFY)).guidance
}

/// The guidance of a consumer that shows notify and each command for which `shown` is
/// true.
fn guidance_of(shown: impl Fn(Command) -> bool) -> String {
    guidance_with(shown, true)
}

/// The guidance of a consumer that shows each command and notify.
fn guidance() -> String {
    guidance_of(|_| true)
}

#[test]
#[provenance_macros::verifies("rule_guidance_names_accepted_commands", examples)]
fn the_guidance_names_no_command_that_the_consumer_does_not_show() {
    for hidden in Command::ALL.iter().copied() {
        let text = guidance_of(|command| command != hidden);
        let name = hidden.declaration().name;
        assert!(
            !names(&command_text(&text), name),
            "the guidance without {name} names it:\n{text}"
        );
    }
    let text = guidance_with(|_| true, false);
    assert!(
        !names(&command_text(&text), NOTIFY),
        "the guidance without the notify write names it:\n{text}"
    );
    let text = guidance();
    assert!(
        text.contains(&format!("; {NOTIFY} to register a client thread")),
        "the guidance does not give the notify write by the name of the consumer:\n{text}"
    );
}

/// The lines follow the order of the consumer and use its names. A command that the
/// consumer declares itself gets a line with its own description. An internal command
/// gets no line.
#[test]
#[provenance_macros::verifies("rule_guidance_names_accepted_commands", examples)]
fn each_shown_command_has_one_line_with_the_name_of_the_consumer() {
    let import = Command::Repository(RepositoryCommand::Import);
    let merge = Command::Repository(RepositoryCommand::MergeJsonl);
    let serve = Declaration::named(
        "serve",
        "Serve the records.",
        Fields::new(&[], serde_json::Value::default),
    );
    let commands = [
        (Shown::Registry(import), "loaded import".to_owned()),
        (Shown::Registry(Command::Search), "find".to_owned()),
        (Shown::Registry(merge), "merge".to_owned()),
        (Shown::Declared(serve), "start".to_owned()),
    ];
    let text = guidance_for(&commands, &[], None).guidance;
    let (_, actions) = text.split_once("\n## Actions\n").unwrap();
    let lines: Vec<&str> = actions
        .lines()
        .filter(|line| line.starts_with("- "))
        .collect();
    assert_eq!(
        lines,
        [
            format!("- loaded import: {}", import.declaration().description),
            format!("- find: {}", Command::Search.declaration().description),
            "- start: Serve the records.".to_owned(),
        ]
    );
    assert!(merge.declaration().internal);
}

#[test]
fn the_create_sentence_names_existing_record_actions_only_when_one_is_shown() {
    let existing = |command: Command| {
        matches!(command, Command::Action(action)
            if Action::RECORD.contains(&action) && action != Action::Create)
    };
    let text = guidance_of(|command| !existing(command));
    assert!(
        text.contains(" Create names an explicit record type.\n"),
        "{text}"
    );
    assert!(
        !text.contains("actions on an existing record infer its type"),
        "{text}"
    );
    assert!(guidance().contains(
        "\nUse search to find IDs, then get to read the selected record and its related views. \
         Create names an explicit record type; actions on an existing record infer its type.\n"
    ));
}

/// No MCP host lists trace without get, because both tools need the same read grant.
/// The shared guidance does not know its consumer, so this porcelain test is the
/// evidence: the trace line names get and goes with it.
#[test]
fn guidance_with_trace_and_without_get_gives_no_trace_line() {
    let without_get = guidance_of(|command| command != Command::Get);
    let without_get_and_trace =
        guidance_of(|command| !matches!(command, Command::Get | Command::Trace));
    assert_eq!(without_get, without_get_and_trace);
    assert!(guidance().contains("\n- trace: "));
}

/// The `edit.etag` sentence names get and update, so a consumer gets it only when it shows
/// the two commands.
#[test]
fn the_etag_sentence_needs_get_and_update() {
    let sentence = "Before an update, read the record with get. Give its `edit.etag` unchanged \
                    to the update as the if-match value.\n";
    let update = Command::Action(Action::Update);
    let without_update = guidance_of(|command| command != update);
    assert!(!without_update.contains(sentence), "{without_update}");
    let without_get = guidance_of(|command| command != Command::Get);
    assert!(!without_get.contains(sentence), "{without_get}");
    let get_and_update = guidance_of(|command| command == Command::Get || command == update);
    assert_eq!(
        get_and_update.matches(sentence).count(),
        1,
        "{get_and_update}"
    );
}

/// With each Discussion read, the description of the discussions command is the text of
/// its declaration, which CLI help shows. Without the list, it is the description of the
/// read of one Discussion. No Rule binds this test: the MCP test of the discussions tool
/// is the evidence for the description that a host gives.
#[test]
fn the_discussions_description_with_each_read_is_the_declared_text() {
    let declared = |action| Command::Action(action).declaration().description;
    assert_eq!(
        discussions_description(|_| true),
        Some(declared(Action::Discussions))
    );
    assert_eq!(
        discussions_description(|action| action == Action::Discussions),
        Some(declared(Action::Discussions))
    );
    assert_eq!(
        discussions_description(|action| action == Action::Discussion),
        Some(declared(Action::Discussion))
    );
    assert_eq!(discussions_description(|_| false), None);
}
