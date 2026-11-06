//! The CLI grammar of the commands that pin graph references and land swarm backtraces.

use super::{json_format_flag, CliForm, Flag, FlagKind, Relation, REPO, SCOPE};
use provenance_porcelain::registry::GraphCommand;

const JSON_FORMAT: Flag = json_format_flag(Some("json"));

const REFERENCE_ISSUE_FLAGS: &[Flag] = &[
    REPO,
    SCOPE,
    Flag::new("commit", FlagKind::Value)
        .help("Git revision to pin. When omitted, clean relevant state at HEAD is required"),
    Flag::new("correlation-system", FlagKind::Value),
    Flag::new("correlation-key", FlagKind::Value),
];

const REFERENCE_FLAGS: &[Flag] = &[REPO, Flag::new("reference", FlagKind::Value).required()];

const SWARM_BACKTRACE_LAND_FLAGS: &[Flag] = &[
    REPO,
    SCOPE,
    Flag::new("run-dir", FlagKind::Value).required(),
    Flag::new("replace", FlagKind::Switch),
    JSON_FORMAT,
];

pub(super) const fn form(command: GraphCommand) -> CliForm {
    match command {
        GraphCommand::ReferenceIssue => CliForm::new(REFERENCE_ISSUE_FLAGS).relations(&[
            Relation::Requires("correlation_system", "correlation_key"),
            Relation::Requires("correlation_key", "correlation_system"),
        ]),
        GraphCommand::ReferenceShow
        | GraphCommand::ReferenceVerify
        | GraphCommand::ReferenceExactExport => CliForm::new(REFERENCE_FLAGS),
        GraphCommand::SwarmBacktraceLand => CliForm::new(SWARM_BACKTRACE_LAND_FLAGS),
    }
}
