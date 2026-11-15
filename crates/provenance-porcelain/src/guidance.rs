//! Domain guidance shared by each consumer.

use crate::action::Action;
use crate::registry::{Command, GraphCommand, RepositoryCommand};
use serde::Serialize;
use std::fmt::Write;

/// The same domain text for each consumer. A consumer adds its own first steps.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct Guidance {
    pub guidance: String,
}

/// The domain text before the action lines.
const DOMAIN: &str = "# Provenance\n\n\
Provenance records requirements, decisions, and the rules that connect them to code.\n\n\
## Records\n\n\
- Requirement: an obligation the system must satisfy. Requirements can refine other Requirements.\n\
- Rule: an identified atomic behavioural obligation that refines one or more Requirements. A Rule can exist before code or verification.\n\
- Resolution: a recorded decision that removes ambiguity. A Resolution can produce a Rule when it establishes a precise obligation.\n\
- Source: identified evidence that graph records can cite. A citation records where an obligation came from.\n\
- Topic: a claimable shaping work area attached to a Requirement.\n\
- Question: a concern to resolve, attached to a Topic and a Requirement, with a selected resolution method.\n\
- Discussion: one concern identified by a root Message and its replies inside a Thread.\n\
- Description: on a Requirement or Rule, how the product behaves in the present tense: states, what changes them, what the person sees, one example. History goes on the thread; unknowns go in a Question.\n\n\
## Code and evidence\n\n\
An Implementation binding connects a Rule to production code that realizes it.\n\
Verification is evidence that a Rule holds. A Verification binding identifies intended evidence; a Verification run records one execution and its result.\n\
Rule lifecycle, decision grounding, implementation, and verification are separate facts. An active Rule can precede its implementation.\n\
Unimplemented and Unverified describe absent bindings or evidence; they are not stored Rule statuses. A graph read does not scan code or establish a coverage verdict.\n\
A source citation does not count as an Implementation binding. Coverage checks evaluate code bindings separately from graph reads.\n\n\
## Actions\n\n";

/// The opening sentence of a consumer that shows search and get.
const SEARCH_THEN_GET: &str =
    "Use search to find IDs, then get to read the selected record and its related views.";

/// The create sentence, and its clause for the actions on an existing record.
const CREATE: (&str, &str) = (
    "Create names an explicit record type",
    "; actions on an existing record infer its type",
);

/// The sentence of a consumer that shows get and update: the `edit.etag` of a get guards
/// the update.
const ETAG_BEFORE_UPDATE: &str = "Before an update, read the record with get. \
Give its `edit.etag` unchanged to the update as the if-match value.\n";

/// The sentence after the opening sentences, which names no command.
const ACCESS: &str = "Available actions depend on the interface and its access. \
Each interface lists the inputs and actions it supports.\n\n";

/// The change description before the writes and after them.
const CHANGE: (&str, &str) = (
    "Write to the graph. The action argument selects the write: ",
    ". A call that lacks a field that the action and record kind need gets the schema of \
     their fields.",
);

/// The notify write. It is not a command: the get command runs it with its notify
/// field.
const NOTIFY: &str = "notify";

/// The Discussion writes of the change command.
const DISCUSSION_WRITES: [Command; 2] = [
    Command::Action(Action::Discuss),
    Command::Action(Action::Reply),
];

/// The repository writes of the change command.
const REPOSITORY_WRITES: [Command; 4] = [
    Command::Repository(RepositoryCommand::Materialize),
    Command::Repository(RepositoryCommand::Import),
    Command::Repository(RepositoryCommand::DictionaryImport),
    Command::Graph(GraphCommand::SwarmBacktraceLand),
];

/// Produce the guidance of a consumer that shows the commands that `name` names.
///
/// `name` gives the name with which the consumer shows a command, or `None` for a command
/// that it does not show. The notify write is shown when `notify` is true. A sentence or a command line names
/// only shown commands, each line with the name of the consumer, and the change line names
/// only the shown writes.
pub fn guidance_for(name: impl Fn(Command) -> Option<&'static str>, notify: bool) -> Guidance {
    let shown = |command: Command| name(command).is_some();
    let mut text = String::from(DOMAIN);
    let opening: Vec<String> = [
        (shown(Command::Search) && shown(Command::Get)).then(|| SEARCH_THEN_GET.to_owned()),
        create_sentence(shown),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !opening.is_empty() {
        text.push_str(&opening.join(" "));
        text.push('\n');
    }
    if shown(Command::Get) && shown(Command::Action(Action::Update)) {
        text.push_str(ETAG_BEFORE_UPDATE);
    }
    text.push_str(ACCESS);
    let commands = [
        Command::Get,
        Command::Trace,
        Command::Search,
        Command::Check,
        Command::Api,
        Command::Change,
    ]
    .into_iter()
    .chain(
        Action::RECORD
            .into_iter()
            .chain(Action::DISCUSSION)
            .map(Command::Action),
    );
    let names_shown = |command: &Command| also_named(*command).iter().all(|other| shown(*other));
    for command in commands.filter(names_shown) {
        let Some(shown_name) = name(command) else {
            continue;
        };
        let description = if command == Command::Change {
            let Some(description) = change_description(&name, notify) else {
                continue;
            };
            description
        } else {
            command.declaration().description.to_owned()
        };
        writeln!(text, "- {shown_name}: {description}")
            .expect("writing to a String cannot fail");
    }
    text.truncate(text.trim_end().len());
    Guidance { guidance: text }
}

/// The create sentence, when create is shown. Its second clause stays only when an
/// action on an existing record is shown.
fn create_sentence(shown: impl Fn(Command) -> bool) -> Option<String> {
    let existing = Action::RECORD
        .into_iter()
        .any(|action| action != Action::Create && shown(Command::Action(action)));
    shown(Command::Action(Action::Create)).then(|| {
        let (sentence, clause) = CREATE;
        format!("{sentence}{}.", if existing { clause } else { "" })
    })
}

/// The description of the change command that names only the shown writes.
///
/// `name` gives the name with which the consumer shows a write, or `None` for a write that
/// it does not show. Notify is shown when `notify` is true. With each write shown with its
/// declaration name, it is the change declaration description. No write gives no
/// description.
pub fn change_description(
    name: impl Fn(Command) -> Option<&'static str>,
    notify: bool,
) -> Option<String> {
    let names = |commands: &[Command]| -> Vec<&'static str> {
        commands.iter().filter_map(|command| name(*command)).collect()
    };
    let groups = [
        (
            names(&Action::RECORD.map(Command::Action)),
            "on the target record",
        ),
        (names(&DISCUSSION_WRITES), "for a Discussion"),
        (
            if notify { vec![NOTIFY] } else { Vec::new() },
            "to register a client thread for review changes",
        ),
        (names(&REPOSITORY_WRITES), "on the repository files"),
    ];
    let mut writes: Vec<String> = groups
        .into_iter()
        .filter(|(names, _)| !names.is_empty())
        .map(|(names, words)| format!("{} {words}", alternatives(&names)))
        .collect();
    if let [_, .., last] = writes.as_mut_slice() {
        last.insert_str(0, "or ");
    }
    let (before, after) = CHANGE;
    (!writes.is_empty()).then(|| format!("{before}{}{after}", writes.join("; ")))
}

/// The names as alternatives: `a`, `a or b`, or `a, b, or c`.
fn alternatives(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [name] => (*name).to_owned(),
        [first, second] => format!("{first} or {second}"),
        [rest @ .., last] => format!("{}, or {last}", rest.join(", ")),
    }
}

/// The other commands that the description of a command names.
const fn also_named(command: Command) -> &'static [Command] {
    match command {
        Command::Trace => &[Command::Get],
        _ => &[],
    }
}
