//! The CLI grammar of the commands that set up, export, import, merge, and check
//! repository files.

use super::{json_format_flag, CliForm, Flag, FlagKind, Relation, REPO, SCOPE};
use provenance_porcelain::registry::RepositoryCommand;

const STE_PDF_HELP: &str =
    "Import this local Issue 9 dictionary PDF instead of downloading the official asset";

const ARTIFACTS: &[&str] = &[
    "contribution",
    "synthesis-packet",
    "proposal",
    "assertion",
    "disposition",
    "graph-reference",
    "graph-reference-export",
];

const JSON_FORMAT: Flag = json_format_flag(Some("json"));
const DEFAULT_REPO: Flag = Flag::new("repo", FlagKind::Value).default(".");

const INIT_FLAGS: &[Flag] = &[
    Flag::new("path", FlagKind::Value).default("."),
    Flag::new("scope", FlagKind::Value),
    Flag::new("path-prefix", FlagKind::Value),
    Flag::new("disposition-actor-id", FlagKind::Repeated).help(
        "Repository-local actor ID allowed to attest proposal dispositions. Repeat for \
         multiple actors. On re-init, omission preserves the allowlist",
    ),
    Flag::new("clear-disposition-actors", FlagKind::Switch)
        .help("Empty the repository-local disposition actor allowlist"),
    Flag::new("ste-pdf", FlagKind::Value).help(STE_PDF_HELP),
    Flag::new("invocation-channel", FlagKind::Value)
        .values(&["native", "typescript"])
        .default("native")
        .hidden(),
    Flag::new("package-manager", FlagKind::Value)
        .values(&["npm", "pnpm", "yarn", "bun", "deno", "nub"])
        .hidden(),
];

const CARGO_INIT_FLAGS: &[Flag] = &[
    Flag::new("package", FlagKind::Value),
    Flag::new("ste-pdf", FlagKind::Value).help(STE_PDF_HELP),
];

const MATERIALIZE_FLAGS: &[Flag] = &[DEFAULT_REPO, JSON_FORMAT];

const EXPORT_FLAGS: &[Flag] = &[
    REPO,
    SCOPE,
    Flag::new("format", FlagKind::Value)
        .values(&["json", "jsonl", "markdown", "table", "toon"])
        .default("json"),
    Flag::new("output", FlagKind::Value),
];

const IMPORT_FLAGS: &[Flag] = &[
    REPO,
    SCOPE,
    Flag::new("input", FlagKind::Value).required(),
    Flag::new("dry-run", FlagKind::Switch),
    JSON_FORMAT,
];

const MERGE_JSONL_FLAGS: &[Flag] = &[
    Flag::new("base", FlagKind::Positional).required(),
    Flag::new("ours", FlagKind::Positional).required(),
    Flag::new("theirs", FlagKind::Positional).required(),
    Flag::new("output", FlagKind::Value),
    Flag::new("path", FlagKind::Value).help(
        "Repository path the merged result belongs at. Git merge drivers get three temporary \
         files, so this is how the merge learns which record type the file holds and which \
         write-time checks to re-apply",
    ),
    JSON_FORMAT,
];

const VALIDATE_FLAGS: &[Flag] = &[
    Flag::new("artifact", FlagKind::Positional)
        .values(ARTIFACTS)
        .required(),
    Flag::new("input", FlagKind::Value).required(),
    JSON_FORMAT,
];

const SCHEMA_SHOW_FLAGS: &[Flag] = &[
    Flag::new("artifact", FlagKind::Positional)
        .values(ARTIFACTS)
        .required(),
    JSON_FORMAT,
];

const DICTIONARY_IMPORT_FLAGS: &[Flag] = &[
    Flag::new("pdf", FlagKind::Value)
        .required()
        .help("Path to the local official Issue 9 PDF file"),
    DEFAULT_REPO,
    JSON_FORMAT,
];

const DICTIONARY_STATUS_FLAGS: &[Flag] = &[DEFAULT_REPO, JSON_FORMAT];

/// The CLI grammar of one repository command. Help does not show the commands that
/// programs run.
#[provenance_macros::rule("rule_cli_help_hides_program_commands")]
pub(super) const fn form(command: RepositoryCommand) -> CliForm {
    match command {
        RepositoryCommand::Init => CliForm::new(INIT_FLAGS).relations(&[
            Relation::Requires("path_prefix", "scope"),
            Relation::Conflicts("clear_disposition_actors", "disposition_actor_id"),
        ]),
        RepositoryCommand::CargoInit => CliForm::new(CARGO_INIT_FLAGS).hidden(),
        RepositoryCommand::Materialize => CliForm::new(MATERIALIZE_FLAGS),
        RepositoryCommand::Export => CliForm::new(EXPORT_FLAGS),
        RepositoryCommand::Import => CliForm::new(IMPORT_FLAGS),
        // Git runs merge-jsonl as a merge driver, so help does not show it.
        RepositoryCommand::MergeJsonl => CliForm::new(MERGE_JSONL_FLAGS)
            .positional(&["base", "ours", "theirs"])
            .hidden(),
        RepositoryCommand::Validate => CliForm::new(VALIDATE_FLAGS).positional(&["artifact"]),
        RepositoryCommand::SchemaShow => CliForm::new(SCHEMA_SHOW_FLAGS).positional(&["artifact"]),
        RepositoryCommand::DictionaryImport => CliForm::new(DICTIONARY_IMPORT_FLAGS),
        RepositoryCommand::DictionaryStatus => CliForm::new(DICTIONARY_STATUS_FLAGS),
    }
}
