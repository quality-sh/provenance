//! The server instructions: the shared guidance without the lines of the tools that the
//! server does not list, and the first calls when the server lists their tools.

use crate::exposed::{self, Place, EXPOSED};

/// The first calls that the server suggests after the shared guidance.
const FIRST_STEPS: &str = "\n\nFor a focused start, call search with {\"text\":\"<text>\"}, \
then get with {\"target\":\"<id>\",\"view\":\"children\",\"depth\":<N>}.";

/// The tools that the first calls use.
const FIRST_STEP_TOOLS: [&str; 2] = ["search", "get"];

/// The sentence of the shared guidance that tells an agent to call search and then get.
const SEARCH_THEN_GET: &str =
    "Use search to find IDs, then get to read the selected record and its related views. ";

/// The instructions of a server that lists each tool for which `listed` is true.
///
/// A guidance line of a command that MCP shows stays only when the server lists the
/// tool that holds the command. The search and get sentence and the first calls stay
/// only when the server lists both tools.
pub fn instructions(listed: impl Fn(&str) -> bool) -> String {
    let first_steps = FIRST_STEP_TOOLS.into_iter().all(&listed);
    let shared = provenance_porcelain::guidance::guidance().guidance;
    let mut text = String::with_capacity(shared.len() + FIRST_STEPS.len());
    for line in shared.split_inclusive('\n') {
        if holding_tool(line).is_some_and(|tool| !listed(tool)) {
            continue;
        }
        if first_steps {
            text.push_str(line);
        } else {
            text.push_str(&line.replacen(SEARCH_THEN_GET, "", 1));
        }
    }
    text.truncate(text.trim_end().len());
    if first_steps {
        text.push_str(FIRST_STEPS);
    }
    text
}

/// The MCP tool that holds the command of one guidance line, when the line describes a
/// command that MCP shows.
fn holding_tool(line: &str) -> Option<&'static str> {
    let line = line.strip_prefix("- ")?;
    EXPOSED.iter().find_map(|(command, place)| {
        let rest = line.strip_prefix(command.declaration().name)?;
        rest.starts_with(": ").then(|| match place {
            Place::Tool => exposed::name(*command),
            Place::Action { tool } | Place::Category { tool } => tool,
        })
    })
}
