//! Declarations of the dev-build-only commands that record and triage pain-point notes
//! about provenance itself.

use super::Declaration;
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

pub(super) const fn declaration(command: DogfoodCommand) -> Declaration {
    match command {
        DogfoodCommand::Note => Declaration::named(
            "dogfood note",
            "Record one pain-point note. Cheap by design: three enums and a sentence",
            note_schema,
        ),
        DogfoodCommand::List => {
            Declaration::named("dogfood list", "Print the local note spool", list_schema)
        }
        DogfoodCommand::Report => Declaration::named(
            "dogfood report",
            "Aggregate the spool; optionally join session metadata supplied by a sister system \
             via the provenance-dogfood-enrichment/v1 contract",
            report_schema,
        ),
        DogfoodCommand::TriageHandle => Declaration::named(
            "dogfood triage handle",
            "Mark one note handled",
            triage_handle_schema,
        ),
        DogfoodCommand::TriageReopen => Declaration::named(
            "dogfood triage reopen",
            "Return a handled note to the unhandled state",
            triage_reopen_schema,
        ),
        DogfoodCommand::TriageList => Declaration::named(
            "dogfood triage list",
            "Show every captured note with its identifier and triage state",
            triage_list_schema,
        ),
        DogfoodCommand::Simulate => Declaration::named(
            "dogfood simulate",
            "Dev-build-only: try the whole first-run experience in a throwaway sandbox. Serves \
             the STE dictionary from loopback, records no note, and touches nothing outside the \
             sandbox",
            simulate_schema,
        ),
    }
}
