//! Declarations of the dev-build-only commands that record and triage pain-point notes
//! about provenance itself.

use super::{json_format, Declaration, Field, Fields};
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

const JSON_FORMAT: Field = json_format(Some("json"));

const NOTE_ID_HELP: &str = "Note id, or an unambiguous prefix of one, as shown by the triage list";

const NOTE_FIELDS: &[Field] = &[
    Field::text("surface")
        .required()
        .doc("The part of provenance the note concerns: a command name, or \"general\""),
    Field::text("category")
        .values(&["friction", "confusion", "missing", "bug", "idea"])
        .required(),
    Field::text("severity")
        .values(&["blocked", "workaround", "annoyance"])
        .required()
        .doc("Impact the pain had on the task at hand"),
    Field::text("detail").doc("What you were trying to do, what happened, what you expected"),
    Field::text("suggestion"),
    Field::text("summary")
        .required()
        .doc("One-line summary of the pain point"),
];

const LIST_FIELDS: &[Field] = &[JSON_FORMAT];

const REPORT_FIELDS: &[Field] = &[
    Field::text("enrich").doc(
        "Enrichment JSON conforming to provenance-dogfood-enrichment/v1 (a file path, or \"-\" \
         for stdin)",
    ),
    JSON_FORMAT,
];

const TRIAGE_HANDLE_FIELDS: &[Field] = &[
    Field::text("id").required().doc(NOTE_ID_HELP),
    Field::text("reason").doc("Why the note is handled: a short reason or an issue reference"),
];

const TRIAGE_REOPEN_FIELDS: &[Field] = &[Field::text("id").required().doc(NOTE_ID_HELP)];

const TRIAGE_LIST_FIELDS: &[Field] = &[
    Field::text("status")
        .values(&["unhandled", "handled", "all"])
        .default("all")
        .doc("Only show notes in this triage state"),
    JSON_FORMAT,
];

const SIMULATE_FIELDS: &[Field] = &[
    Field::switch("keep").doc("Keep the sandbox directory after the run for inspection"),
    Field::text("dir").doc(
        "Create the sandbox inside this existing directory instead of the system temp location",
    ),
];

pub(super) const fn declaration(command: DogfoodCommand) -> Declaration {
    match command {
        DogfoodCommand::Note => Declaration::named(
            "dogfood note",
            "Record one pain-point note. Cheap by design: three enums and a sentence",
            Fields::new(NOTE_FIELDS, note_schema).target(&["summary"]),
        ),
        DogfoodCommand::List => Declaration::named(
            "dogfood list",
            "Print the local note spool",
            Fields::new(LIST_FIELDS, list_schema),
        ),
        DogfoodCommand::Report => Declaration::named(
            "dogfood report",
            "Aggregate the spool; optionally join session metadata supplied by a sister system \
             via the provenance-dogfood-enrichment/v1 contract",
            Fields::new(REPORT_FIELDS, report_schema),
        ),
        DogfoodCommand::TriageHandle => Declaration::named(
            "dogfood triage handle",
            "Mark one note handled",
            Fields::new(TRIAGE_HANDLE_FIELDS, triage_handle_schema).target(&["id"]),
        ),
        DogfoodCommand::TriageReopen => Declaration::named(
            "dogfood triage reopen",
            "Return a handled note to the unhandled state",
            Fields::new(TRIAGE_REOPEN_FIELDS, triage_reopen_schema).target(&["id"]),
        ),
        DogfoodCommand::TriageList => Declaration::named(
            "dogfood triage list",
            "Show every captured note with its identifier and triage state",
            Fields::new(TRIAGE_LIST_FIELDS, triage_list_schema),
        ),
        DogfoodCommand::Simulate => Declaration::named(
            "dogfood simulate",
            "Dev-build-only: try the whole first-run experience in a throwaway sandbox. Serves \
             the STE dictionary from loopback, records no note, and touches nothing outside the \
             sandbox",
            Fields::new(SIMULATE_FIELDS, simulate_schema),
        ),
    }
}
