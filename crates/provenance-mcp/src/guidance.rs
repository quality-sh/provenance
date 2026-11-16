//! The server instructions: the shared guidance of the commands that the server shows,
//! and the first calls when the server shows their commands.

use crate::listed::Table;
use provenance_porcelain::guidance::{first_steps, guidance_for, Shown};
use provenance_porcelain::registry::{Command, Field, FieldValue, TARGET};
use std::fmt::Write;

/// The text before the first calls, between two calls, and after the last call.
const FIRST_STEPS: (&str, &str, &str) = ("\n\nFor a focused start, call ", ", then ", ".");

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
    let calls: Vec<String> = first_steps(shown)
        .map(|(command, fields)| call(command, fields))
        .collect();
    if !calls.is_empty() {
        let (before, between, after) = FIRST_STEPS;
        write!(text, "{before}{}{after}", calls.join(between))
            .expect("writing to a String cannot fail");
    }
    text
}

/// One first call: the tool name of `command` and its input, with the target and `fields`.
/// A field has its selected value, or a placeholder.
fn call(command: Command, fields: &[(Field, Option<&str>)]) -> String {
    let target = command
        .declaration()
        .grammar
        .takes_target()
        .then(|| format!("\"{}\":\"<id>\"", TARGET.name));
    let fields = fields.iter().map(|(field, value)| {
        let value = match (value, field.value) {
            (Some(value), _) => format!("\"{value}\""),
            (None, FieldValue::Count) => "<N>".to_owned(),
            (None, _) => format!("\"<{}>\"", field.name),
        };
        format!("\"{}\":{value}", field.name)
    });
    let input: Vec<String> = target.into_iter().chain(fields).collect();
    format!(
        "{} with {{{}}}",
        crate::exposed::name(command),
        input.join(",")
    )
}
