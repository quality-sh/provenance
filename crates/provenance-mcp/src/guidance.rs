//! The server instructions: the shared guidance of the commands that the server shows,
//! and the first calls when the server shows their commands.

use provenance_porcelain::registry::Command;

/// The first calls that the server suggests after the shared guidance.
const FIRST_STEPS: &str = "\n\nFor a focused start, call search with {\"text\":\"<text>\"}, \
then get with {\"target\":\"<id>\",\"view\":\"children\",\"depth\":<N>}.";

/// The commands that the first calls use.
const FIRST_STEP_COMMANDS: [Command; 2] = [Command::Search, Command::Get];

/// The instructions of a server that shows each command for which `shown` is true, and
/// the notify write when `notify` is true. Each command has its MCP name.
pub fn instructions(shown: impl Fn(Command) -> bool, notify: bool) -> String {
    let name = |command: Command| shown(command).then(|| crate::exposed::name(command));
    let mut text = provenance_porcelain::guidance::guidance_for(name, notify).guidance;
    if FIRST_STEP_COMMANDS.into_iter().all(&shown) {
        text.push_str(FIRST_STEPS);
    }
    text
}
