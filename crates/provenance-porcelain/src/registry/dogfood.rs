//! Declarations of the dev-build-only commands that record and triage pain-point notes
//! about provenance itself. The registry lists them only with the `dogfood` feature.

use super::{json_format_flag, CliForm, Declaration, Flag, FlagKind, Input, Interface, Presence};
use serde::Deserialize;

/// One dev-build-only dogfood command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DogfoodCommand {
    Note,
    List,
    Report,
    TriageHandle,
    TriageReopen,
    TriageList,
    Simulate,
}

impl DogfoodCommand {
    /// Whether `interface` shows this command. Each interface has one exhaustive match.
    pub(super) const fn presence(self, interface: Interface) -> Presence {
        match interface {
            Interface::Cli => match self {
                Self::Note
                | Self::List
                | Self::Report
                | Self::TriageHandle
                | Self::TriageReopen
                | Self::TriageList
                | Self::Simulate => {
                    if cfg!(feature = "dogfood") {
                        Presence::Present
                    } else {
                        Presence::Absent {
                            reason: "only dev builds with the dogfood feature have the command",
                        }
                    }
                }
            },
            Interface::Mcp => match self {
                Self::Note
                | Self::List
                | Self::Report
                | Self::TriageHandle
                | Self::TriageReopen
                | Self::TriageList
                | Self::Simulate => Presence::Absent {
                    reason: "the notes are about the CLI on the local machine",
                },
            },
        }
    }
}

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

/// The `dogfood note` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DogfoodNoteInput {
    pub surface: String,
    pub category: String,
    pub severity: String,
    pub detail: Option<String>,
    pub suggestion: Option<String>,
    pub summary: String,
}

/// The `dogfood list` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DogfoodListInput {
    pub format: String,
}

/// The `dogfood report` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DogfoodReportInput {
    pub enrich: Option<String>,
    pub format: String,
}

/// The `dogfood triage handle` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TriageHandleInput {
    pub id: String,
    pub reason: Option<String>,
}

/// The `dogfood triage reopen` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TriageReopenInput {
    pub id: String,
}

/// The `dogfood triage list` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TriageListInput {
    pub status: String,
    pub format: String,
}

/// The `dogfood simulate` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DogfoodSimulateInput {
    #[serde(default)]
    pub keep: bool,
    pub dir: Option<String>,
}

macro_rules! schema_fn {
    ($name:ident, $input:ty) => {
        fn $name() -> serde_json::Value {
            serde_json::to_value(schemars::schema_for!($input)).expect("schema is JSON")
        }
    };
}

schema_fn!(note_schema, DogfoodNoteInput);
schema_fn!(list_schema, DogfoodListInput);
schema_fn!(report_schema, DogfoodReportInput);
schema_fn!(triage_handle_schema, TriageHandleInput);
schema_fn!(triage_reopen_schema, TriageReopenInput);
schema_fn!(triage_list_schema, TriageListInput);
schema_fn!(simulate_schema, DogfoodSimulateInput);

/// A shown CLI-only command without flag relations.
const fn cli_only(
    name: &'static str,
    words: &'static [&'static str],
    description: &'static str,
    schema: fn() -> serde_json::Value,
    flags: &'static [Flag],
) -> Declaration {
    Declaration {
        name,
        description,
        cli_form: CliForm::Words(words),
        cli_input: Input::Typed(schema),
        mcp_input: Input::None,
        positional: &[],
        relations: &[],
        hidden: false,
        flags,
    }
}

pub(super) fn declaration(command: DogfoodCommand) -> Declaration {
    match command {
        DogfoodCommand::Note => Declaration {
            positional: &["summary"],
            ..cli_only(
                "dogfood note",
                &["dogfood", "note"],
                "Record one pain-point note. Cheap by design: three enums and a sentence",
                note_schema,
                NOTE_FLAGS,
            )
        },
        DogfoodCommand::List => cli_only(
            "dogfood list",
            &["dogfood", "list"],
            "Print the local note spool",
            list_schema,
            LIST_FLAGS,
        ),
        DogfoodCommand::Report => cli_only(
            "dogfood report",
            &["dogfood", "report"],
            "Aggregate the spool; optionally join session metadata supplied by a sister system \
             via the provenance-dogfood-enrichment/v1 contract",
            report_schema,
            REPORT_FLAGS,
        ),
        DogfoodCommand::TriageHandle => Declaration {
            positional: &["id"],
            ..cli_only(
                "dogfood triage handle",
                &["dogfood", "triage", "handle"],
                "Mark one note handled",
                triage_handle_schema,
                TRIAGE_HANDLE_FLAGS,
            )
        },
        DogfoodCommand::TriageReopen => Declaration {
            positional: &["id"],
            ..cli_only(
                "dogfood triage reopen",
                &["dogfood", "triage", "reopen"],
                "Return a handled note to the unhandled state",
                triage_reopen_schema,
                TRIAGE_REOPEN_FLAGS,
            )
        },
        DogfoodCommand::TriageList => cli_only(
            "dogfood triage list",
            &["dogfood", "triage", "list"],
            "Show every captured note with its identifier and triage state",
            triage_list_schema,
            TRIAGE_LIST_FLAGS,
        ),
        DogfoodCommand::Simulate => cli_only(
            "dogfood simulate",
            &["dogfood", "simulate"],
            "Dev-build-only: try the whole first-run experience in a throwaway sandbox. Serves \
             the STE dictionary from loopback, records no note, and touches nothing outside the \
             sandbox",
            simulate_schema,
            SIMULATE_FLAGS,
        ),
    }
}
