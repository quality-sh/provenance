//! The server instructions: the shared guidance of the commands that the server shows,
//! and the first calls when the server shows their commands.

use crate::listed::Table;
use provenance_porcelain::guidance::{first_steps, guidance_for, FirstStep, Shown};
use provenance_porcelain::registry::{Command, FieldValue, Grammar};

/// The text before the first calls.
const BEFORE_CALLS: &str = "\n\nFor a focused start, call ";
/// The text between two first calls.
const BETWEEN_CALLS: &str = ", then ";
/// The text after the last first call.
const AFTER_CALLS: &str = ".";

/// The instructions of a server that shows the commands of `table`, each with its MCP
/// name, and the first calls when the server shows their commands.
pub fn instructions(table: &Table) -> String {
    let name = |command: Command| crate::exposed::name(command).to_owned();
    let commands: Vec<(Shown, String)> = table
        .commands()
        .map(|command| (Shown::Registry(command), name(command)))
        .collect();
    let writes: Vec<(Command, String)> = table
        .writes()
        .into_iter()
        .map(|command| (command, name(command)))
        .collect();
    let guidance = guidance_for(&commands, &writes, table.notify());
    let mut text = guidance.guidance;
    let shown = |command: Command| table.commands().any(|other| other == command);
    let calls: Vec<String> = first_steps(shown).iter().map(call).collect();
    if !calls.is_empty() {
        text.push_str(BEFORE_CALLS);
        text.push_str(&calls.join(BETWEEN_CALLS));
        text.push_str(AFTER_CALLS);
    }
    text
}

/// One first call: the tool name of the command of `step` and its input, with the target
/// and the arguments of the step. A count is a JSON number and each other value a string.
fn call(step: &FirstStep) -> String {
    let target = step
        .command
        .declaration()
        .grammar
        .takes_target()
        .then(|| format!("\"{}\":\"<id>\"", Grammar::TARGET));
    let arguments = step.arguments.iter().map(|argument| {
        let value = argument.value();
        let value = match argument.field.value {
            FieldValue::Count => value.into_owned(),
            _ => format!("\"{value}\""),
        };
        format!("\"{}\":{value}", argument.field.name)
    });
    let input: Vec<String> = target.into_iter().chain(arguments).collect();
    format!(
        "{} with {{{}}}",
        crate::exposed::name(step.command),
        input.join(",")
    )
}
