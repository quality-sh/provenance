//! The server instructions: the shared guidance of the commands that the server shows,
//! and the first calls when the server shows their commands.

use crate::listed::Table;
use provenance_porcelain::registry::Command;

/// The first calls that the server suggests after the shared guidance.
const FIRST_STEPS: &str = "\n\nFor a focused start, call search with {\"text\":\"<text>\"}, \
then get with {\"target\":\"<id>\",\"view\":\"children\",\"depth\":<N>}.";

/// The commands that the first calls use.
const FIRST_STEP_COMMANDS: [Command; 2] = [Command::Search, Command::Get];

/// The instructions of a server that shows the commands of `table`, each with its MCP
/// name, and the first calls when the server shows their commands.
pub fn instructions(table: &Table) -> String {
    let named = |commands: Vec<Command>| -> Vec<(Command, String)> {
        commands
            .into_iter()
            .map(|command| (command, crate::exposed::name(command).to_owned()))
            .collect()
    };
    let commands = named(table.commands().collect());
    let writes = named(table.writes());
    let notify = table.shows_notify();
    let guidance = provenance_porcelain::guidance::guidance_for(&commands, &writes, notify);
    let mut text = guidance.guidance;
    let shown = |command: Command| commands.iter().any(|(other, _)| *other == command);
    if FIRST_STEP_COMMANDS.into_iter().all(shown) {
        text.push_str(FIRST_STEPS);
    }
    text
}
