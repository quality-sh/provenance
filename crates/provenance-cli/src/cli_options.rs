//! The CLI's own options and words. The CLI shows each porcelain field as a
//! `--kebab-case` flag and each target field as a positional argument; the options here
//! are the CLI's, not fields of a command.

use provenance_porcelain::registry::{json_format, Declaration, Field, REPO, SCOPE};

/// The repository options that the porcelain commands and the commands after them take.
pub const GLOBAL_OPTIONS: &[Field] = &[
    REPO.leading_hyphen(),
    SCOPE.leading_hyphen(),
    json_format(None),
    Field::switch("quiet"),
];

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
