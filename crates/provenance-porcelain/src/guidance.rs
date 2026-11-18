//! Domain guidance shared by each consumer.

use crate::action::Action;
use crate::registry::{Command, Declaration, Field, CHILDREN, DEPTH, SEARCH_TEXT, VIEW};
use provenance_macros::rule;
use serde::Serialize;
use std::borrow::Cow;
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
- Description: on a Requirement or Rule, how the product behaves in the present tense: states, what changes them, what the person sees, and an example only when it shows a case that the other sentences do not. History goes on the thread; unknowns go in a Question.\n\n\
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

/// The change description after the change declaration description: the text before the
/// writes and after them.
const CHANGE: (&str, &str) = (
    "The action argument selects the write: ",
    ". A call that lacks a field that the action and record kind need gets the schema of \
     their fields.",
);

/// A first step: a command, and the arguments that it gives after its target.
#[derive(Clone, Copy, Debug)]
pub struct FirstStep {
    pub command: Command,
    pub arguments: &'static [Argument],
}

/// An argument of a first step: a field, and the value that the step selects for it, when
/// it selects one.
#[derive(Clone, Copy, Debug)]
pub struct Argument {
    pub field: Field,
    selected: Option<&'static str>,
}

impl Argument {
    /// The value of the argument: the value that the step selects, or else the placeholder
    /// of the field.
    pub fn value(&self) -> Cow<'static, str> {
        self.selected
            .map_or_else(|| self.field.placeholder(), Cow::Borrowed)
    }
}

/// The first steps: search on a text, then get of the records below one record.
pub const FIRST_STEPS: &[FirstStep] = &[
    FirstStep {
        command: Command::Search,
        arguments: &[Argument {
            field: SEARCH_TEXT,
            selected: None,
        }],
    },
    FirstStep {
        command: Command::Get,
        arguments: &[
            Argument {
                field: VIEW,
                selected: Some(CHILDREN),
            },
            Argument {
                field: DEPTH,
                selected: None,
            },
        ],
    },
];

/// A command that an interface shows: a registry command, or a command that the interface
/// declares itself.
#[derive(Clone, Copy, Debug)]
pub enum Shown {
    Registry(Command),
    Declared(Declaration),
}

impl Shown {
    /// The declaration of the command.
    pub fn declaration(self) -> Declaration {
        match self {
            Self::Registry(command) => command.declaration(),
            Self::Declared(declaration) => declaration,
        }
    }
}

/// Produce the guidance of an interface: a line for each command that it shows, with its
/// name there and its description, and a change line that names `writes` and `notify`.
#[rule("rule_guidance_names_accepted_commands")]
pub fn guidance_for(
    commands: &[(Shown, String)],
    writes: &[(Command, String)],
    notify: Option<&str>,
) -> Guidance {
    let shown = |command: Command| {
        commands
            .iter()
            .any(|(other, _)| matches!(other, Shown::Registry(other) if *other == command))
    };
    let mut text = String::from(DOMAIN);
    let opening: Vec<String> = [
        (!first_steps(shown).is_empty()).then(|| SEARCH_THEN_GET.to_owned()),
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
    for (command, name) in commands {
        let declaration = command.declaration();
        let names_unshown = match command {
            Shown::Registry(command) => !also_named(*command).iter().all(|other| shown(*other)),
            Shown::Declared(_) => false,
        };
        if declaration.internal || names_unshown {
            continue;
        }
        let description = if matches!(command, Shown::Registry(Command::Change)) {
            let Some(description) = change_description(writes, notify) else {
                continue;
            };
            description
        } else {
            declaration.description.to_owned()
        };
        writeln!(text, "- {name}: {description}").expect("writing to a String cannot fail");
    }
    text.truncate(text.trim_end().len());
    Guidance { guidance: text }
}

/// The first steps of an interface that shows each command for which `shown` is true.
///
/// An interface that shows the command of each step has [`FIRST_STEPS`]; another interface
/// has no first steps.
pub fn first_steps(shown: impl Fn(Command) -> bool) -> &'static [FirstStep] {
    if FIRST_STEPS.iter().all(|step| shown(step.command)) {
        FIRST_STEPS
    } else {
        &[]
    }
}

/// The description of the discussions command with the Discussion reads of `granted`: the
/// description of the read that the command selects without an action argument. No granted
/// read gives no description.
pub fn discussions_description(granted: impl Fn(Action) -> bool) -> Option<&'static str> {
    crate::discussion::default_read(granted)
        .map(|action| Command::Action(action).declaration().description)
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

/// The description of a change command that selects `writes`, each with the name that
/// selects it, and the notify write by the name in `notify`.
///
/// The writes are in groups: the record actions, the other actions, which are the
/// Discussion writes, and the repository writes. Each group keeps the order of `writes`.
/// No write gives no description.
pub fn change_description(writes: &[(Command, String)], notify: Option<&str>) -> Option<String> {
    let group = |selected: fn(Command) -> bool| -> Vec<&str> {
        writes
            .iter()
            .filter(|(command, _)| selected(*command))
            .map(|(_, name)| name.as_str())
            .collect()
    };
    let groups = [
        (group(is_record_action), "on the target record"),
        (group(is_discussion_write), "for a Discussion"),
        (
            notify.into_iter().collect(),
            "to register a client thread for review changes",
        ),
        (
            group(|command| !matches!(command, Command::Action(_))),
            "on the repository files",
        ),
    ];
    let mut parts: Vec<String> = groups
        .into_iter()
        .filter(|(names, _)| !names.is_empty())
        .map(|(names, words)| format!("{} {words}", alternatives(&names)))
        .collect();
    if let [_, .., last] = parts.as_mut_slice() {
        last.insert_str(0, "or ");
    }
    let (before, after) = CHANGE;
    let declared = Command::Change.declaration().description;
    (!parts.is_empty()).then(|| format!("{declared} {before}{}{after}", parts.join("; ")))
}

/// Whether `command` is an action on a record.
fn is_record_action(command: Command) -> bool {
    matches!(command, Command::Action(action) if Action::RECORD.contains(&action))
}

/// Whether `command` is an action that is not an action on a record.
fn is_discussion_write(command: Command) -> bool {
    matches!(command, Command::Action(_)) && !is_record_action(command)
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
