//! The CLI's own options and words. The CLI shows each porcelain field as a
//! `--kebab-case` flag and each target field as a positional argument; the options here
//! are the CLI's, not fields of a command.

use provenance_porcelain::registry::{json_format, Declaration, Field, Relation, REPO, SCOPE};

/// The repository options that the porcelain commands and the commands after them take.
pub const GLOBAL_OPTIONS: &[Field] = &[
    REPO.leading_hyphen(),
    SCOPE.leading_hyphen(),
    json_format(None),
    Field::switch("quiet"),
];

/// The CLI's own options of get.
///
/// They change what the terminal does with the answer, not the answer: `follow` keeps
/// the command open and prints each new review document, and `no_open` prints the
/// review link and does not open a browser.
pub const GET_OPTIONS: &[Field] = &[
    Field::switch("follow")
        .doc("Stream the current review document and pushed changes as JSON lines."),
    Field::switch("no_open").doc("Print the review link and do not open it in a browser."),
];

/// The relations of the get options with each other and with the get fields.
pub const GET_OPTION_RELATIONS: &[Relation] = &[
    Relation::Requires("follow", "view"),
    Relation::Conflicts("follow", "review_link"),
    Relation::Conflicts("follow", "notify"),
    Relation::Requires("no_open", "review_link"),
];

/// The CLI's own options of one command, with their relations.
pub fn command_options(declaration: &Declaration) -> (&'static [Field], &'static [Relation]) {
    if declaration.name == "get" {
        (GET_OPTIONS, GET_OPTION_RELATIONS)
    } else {
        (&[], &[])
    }
}

/// The description of each CLI command group that has one, by its words.
pub const GROUP_DESCRIPTIONS: &[(&str, &str)] = &[
    (
        "report",
        "Deterministic report rendering from a versioned report envelope",
    ),
    (
        "dogfood",
        "Dev-build-only: record pain-point notes about provenance itself",
    ),
    (
        "dogfood triage",
        "Dev-build-only triage of captured notes: mark them handled or reopen them. State \
         lives beside the spool; the capture history is never rewritten",
    ),
];

/// The words of `provenance <words...>`, when the name selects the command.
pub fn words(declaration: &Declaration) -> Option<Vec<&'static str>> {
    declaration
        .grammar
        .takes_name()
        .then(|| declaration.words().collect())
}

/// The CLI flag of a field: its name with `-` for `_`.
pub fn flag_name(field: &Field) -> String {
    field.name.replace('_', "-")
}
