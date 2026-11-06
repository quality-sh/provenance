//! The CLI grammar of the dev-build-only commands that record and triage pain-point
//! notes about provenance itself.

use super::{json_format_flag, CliForm, Flag, FlagKind};
use provenance_porcelain::registry::DogfoodCommand;

const JSON_FORMAT: Flag = json_format_flag(Some("json"));

const NOTE_ID_HELP: &str =
    "Note id, or an unambiguous prefix of one, as shown by `provenance dogfood triage list`";

const NOTE_FLAGS: &[Flag] = &[
    Flag::new("surface", FlagKind::Value)
        .required()
        .help("The part of provenance the note concerns: a subcommand name, or \"general\""),
    Flag::new("category", FlagKind::Value)
        .values(&["friction", "confusion", "missing", "bug", "idea"])
        .required(),
    Flag::new("severity", FlagKind::Value)
        .values(&["blocked", "workaround", "annoyance"])
        .required()
        .help("Impact the pain had on the task at hand"),
    Flag::new("detail", FlagKind::Value)
        .help("What you were trying to do, what happened, what you expected"),
    Flag::new("suggestion", FlagKind::Value),
    Flag::new("summary", FlagKind::Positional)
        .required()
        .help("One-line summary of the pain point"),
];

const LIST_FLAGS: &[Flag] = &[JSON_FORMAT];

const REPORT_FLAGS: &[Flag] = &[
    Flag::new("enrich", FlagKind::Value).help(
        "Enrichment JSON conforming to provenance-dogfood-enrichment/v1 (a file path, or \"-\" \
         for stdin)",
    ),
    JSON_FORMAT,
];

const TRIAGE_HANDLE_FLAGS: &[Flag] = &[
    Flag::new("id", FlagKind::Positional)
        .required()
        .help(NOTE_ID_HELP),
    Flag::new("reason", FlagKind::Value)
        .help("Why the note is handled: a short reason or an issue reference"),
];

const TRIAGE_REOPEN_FLAGS: &[Flag] = &[Flag::new("id", FlagKind::Positional)
    .required()
    .help(NOTE_ID_HELP)];

const TRIAGE_LIST_FLAGS: &[Flag] = &[
    Flag::new("status", FlagKind::Value)
        .values(&["unhandled", "handled", "all"])
        .default("all")
        .help("Only show notes in this triage state"),
    JSON_FORMAT,
];

const SIMULATE_FLAGS: &[Flag] = &[
    Flag::new("keep", FlagKind::Switch)
        .help("Keep the sandbox directory after the run for inspection"),
    Flag::new("dir", FlagKind::Value).help(
        "Create the sandbox inside this existing directory instead of the system temp location",
    ),
];

pub(super) const fn form(command: DogfoodCommand) -> CliForm {
    match command {
        DogfoodCommand::Note => CliForm::new(NOTE_FLAGS).positional(&["summary"]),
        DogfoodCommand::List => CliForm::new(LIST_FLAGS),
        DogfoodCommand::Report => CliForm::new(REPORT_FLAGS),
        DogfoodCommand::TriageHandle => CliForm::new(TRIAGE_HANDLE_FLAGS).positional(&["id"]),
        DogfoodCommand::TriageReopen => CliForm::new(TRIAGE_REOPEN_FLAGS).positional(&["id"]),
        DogfoodCommand::TriageList => CliForm::new(TRIAGE_LIST_FLAGS),
        DogfoodCommand::Simulate => CliForm::new(SIMULATE_FLAGS),
    }
}
