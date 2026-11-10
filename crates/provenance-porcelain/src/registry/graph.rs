//! Declarations of the commands that land swarm backtraces.

use super::{Declaration, Field, Fields};
use crate::swarm_backtrace::SwarmBacktraceLandInput;

/// One graph command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphCommand {
    SwarmBacktraceLand,
}

fn swarm_backtrace_land_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(SwarmBacktraceLandInput)).expect("schema is JSON")
}

const SWARM_BACKTRACE_LAND_FIELDS: &[Field] = &[
    Field::text("run_dir")
        .required()
        .doc("The swarm run directory: its extractors, refuters, contributions, and merge output."),
    Field::switch("replace")
        .doc("Replace the generated records that an earlier landing of this run wrote."),
];

pub(super) const fn declaration(command: GraphCommand) -> Declaration {
    match command {
        GraphCommand::SwarmBacktraceLand => Declaration::named(
            "swarm-backtrace land",
            "Add the results of one swarm backtrace run to the graph as proposals",
            Fields::new(SWARM_BACKTRACE_LAND_FIELDS, swarm_backtrace_land_schema),
        )
        .bound(),
    }
}
