//! Domain guidance shared by each consumer.

use crate::action::Action;
use crate::registry::Command;
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

/// The opening sentences of the actions, each with the commands that it names.
const OPENING: [(&str, &[Command]); 2] = [
    (
        "Use search to find IDs, then get to read the selected record and its related views.",
        &[Command::Search, Command::Get],
    ),
    (
        "Create names an explicit record type; actions on an existing record infer its type.",
        &[Command::Action(Action::Create)],
    ),
];

/// The sentence after the opening sentences, which names no command.
const ACCESS: &str = "Available actions depend on the interface and its access. \
Each interface lists the inputs and actions it supports.\n\n";

/// Produce guidance without a repository, a graph query, or a cache.
pub fn guidance() -> Guidance {
    guidance_for(|_| true)
}

/// Produce the guidance of a consumer that shows each command for which `shown` is
/// true. A sentence or a command line stays only when each command that it names is
/// shown.
pub fn guidance_for(shown: impl Fn(Command) -> bool) -> Guidance {
    let all_shown = |commands: &[Command]| commands.iter().all(|command| shown(*command));
    let mut text = String::from(DOMAIN);
    let opening: Vec<&str> = OPENING
        .iter()
        .filter(|(_, named)| all_shown(named))
        .map(|(sentence, _)| *sentence)
        .collect();
    if !opening.is_empty() {
        text.push_str(&opening.join(" "));
        text.push('\n');
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
    for command in commands.filter(|command| shown(*command) && all_shown(also_named(*command))) {
        let declaration = command.declaration();
        writeln!(text, "- {}: {}", declaration.name, declaration.description)
            .expect("writing to a String cannot fail");
    }
    text.truncate(text.trim_end().len());
    Guidance { guidance: text }
}

/// The other commands that the description of a command names.
const fn also_named(command: Command) -> &'static [Command] {
    match command {
        Command::Trace => &[Command::Get],
        _ => &[],
    }
}
