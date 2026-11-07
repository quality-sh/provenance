//! Declarations of the commands that set up, export, import, merge, and check
//! repository files.

use super::{json_format, Declaration, Field, Fields, Relation, REPO, SCOPE};
use crate::dictionary::DictionaryImportInput;
use crate::import::ImportInput;
use crate::materialize::MaterializeInput;
use serde::Deserialize;

/// One repository command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryCommand {
    Init,
    CargoInit,
    Materialize,
    Export,
    Import,
    MergeJsonl,
    Validate,
    SchemaShow,
    DictionaryImport,
    DictionaryStatus,
}

/// The `init` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InitInput {
    pub path: String,
    pub scope: Option<String>,
    pub path_prefix: Option<String>,
    #[serde(default)]
    pub disposition_actor_id: Vec<String>,
    #[serde(default)]
    pub clear_disposition_actors: bool,
    pub ste_pdf: Option<String>,
    pub invocation_channel: String,
    pub package_manager: Option<String>,
}

/// The `__cargo-init` input that the cargo plugin passes on.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CargoInitInput {
    pub package: Option<String>,
    pub ste_pdf: Option<String>,
}

/// The `export` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportInput {
    pub repo: String,
    pub scope: String,
    pub format: String,
    pub output: Option<String>,
}

/// The `merge-jsonl` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MergeJsonlInput {
    pub base: String,
    pub ours: String,
    pub theirs: String,
    pub output: Option<String>,
    pub path: Option<String>,
    pub format: String,
}

/// The `validate` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidateInput {
    pub artifact: String,
    pub input: String,
    pub format: String,
}

/// The `schema show` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SchemaShowInput {
    pub artifact: String,
    pub format: String,
}

/// The `dictionary status` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DictionaryStatusInput {
    pub repo: String,
    pub format: String,
}

// An input without fields has an empty properties object, as each other input has.
macro_rules! schema_fn {
    ($name:ident, $input:ty) => {
        fn $name() -> serde_json::Value {
            let mut schema =
                serde_json::to_value(schemars::schema_for!($input)).expect("schema is JSON");
            schema
                .as_object_mut()
                .expect("an input schema is an object")
                .entry("properties")
                .or_insert_with(|| serde_json::json!({}));
            schema
        }
    };
}

schema_fn!(init_schema, InitInput);
schema_fn!(cargo_init_schema, CargoInitInput);
schema_fn!(materialize_schema, MaterializeInput);
schema_fn!(export_schema, ExportInput);
schema_fn!(import_schema, ImportInput);
schema_fn!(merge_jsonl_schema, MergeJsonlInput);
schema_fn!(validate_schema, ValidateInput);
schema_fn!(schema_show_schema, SchemaShowInput);
schema_fn!(dictionary_import_schema, DictionaryImportInput);
schema_fn!(dictionary_status_schema, DictionaryStatusInput);

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

const JSON_FORMAT: Field = json_format(Some("json"));
const DEFAULT_REPO: Field = Field::text("repo").default(".");

const INIT_FIELDS: &[Field] = &[
    Field::text("path").default("."),
    Field::text("scope"),
    Field::text("path_prefix"),
    Field::repeated("disposition_actor_id").doc(
        "Repository-local actor ID allowed to attest proposal dispositions. Repeat for \
         multiple actors. On re-init, omission preserves the allowlist",
    ),
    Field::switch("clear_disposition_actors")
        .doc("Empty the repository-local disposition actor allowlist"),
    Field::text("ste_pdf").doc(STE_PDF_HELP),
    Field::text("invocation_channel")
        .values(&["native", "typescript"])
        .default("native")
        .internal(),
    Field::text("package_manager")
        .values(&["npm", "pnpm", "yarn", "bun", "deno", "nub"])
        .internal(),
];

const CARGO_INIT_FIELDS: &[Field] = &[
    Field::text("package"),
    Field::text("ste_pdf").doc(STE_PDF_HELP),
];

const MATERIALIZE_FIELDS: &[Field] = &[];

const EXPORT_FIELDS: &[Field] = &[
    REPO,
    SCOPE,
    Field::text("format")
        .values(&["json", "jsonl", "markdown", "table", "toon"])
        .default("json"),
    Field::text("output"),
];

const IMPORT_FIELDS: &[Field] = &[
    Field::text("input")
        .required()
        .doc("The scope export file to import: the JSON document that export writes."),
    Field::switch("dry_run").doc("Run each check of the import and write nothing."),
];

const MERGE_JSONL_FIELDS: &[Field] = &[
    Field::text("base").required(),
    Field::text("ours").required(),
    Field::text("theirs").required(),
    Field::text("output"),
    Field::text("path").doc(
        "Repository path the merged result belongs at. Git merge drivers get three temporary \
         files, so this is how the merge learns which record type the file holds and which \
         write-time checks to re-apply",
    ),
    JSON_FORMAT,
];

const VALIDATE_FIELDS: &[Field] = &[
    Field::text("artifact").values(ARTIFACTS).required(),
    Field::text("input").required(),
    JSON_FORMAT,
];

const SCHEMA_SHOW_FIELDS: &[Field] = &[
    Field::text("artifact").values(ARTIFACTS).required(),
    JSON_FORMAT,
];

const DICTIONARY_IMPORT_FIELDS: &[Field] = &[Field::text("pdf")
    .required()
    .doc("Path to the local official Issue 9 PDF file")];

const DICTIONARY_STATUS_FIELDS: &[Field] = &[DEFAULT_REPO, JSON_FORMAT];

/// The declaration of one repository command. Programs run `__cargo-init` and
/// `merge-jsonl`, so they are internal.
#[provenance_macros::rule("rule_cli_help_hides_program_commands")]
pub(super) const fn declaration(command: RepositoryCommand) -> Declaration {
    match command {
        RepositoryCommand::Init => Declaration::named(
            "init",
            "",
            Fields::new(INIT_FIELDS, init_schema).relations(&[
                Relation::Requires("path_prefix", "scope"),
                Relation::Conflicts("clear_disposition_actors", "disposition_actor_id"),
            ]),
        ),
        RepositoryCommand::CargoInit => Declaration::named(
            "__cargo-init",
            "",
            Fields::new(CARGO_INIT_FIELDS, cargo_init_schema),
        )
        .internal(),
        RepositoryCommand::Materialize => Declaration::named(
            "materialize",
            "",
            Fields::new(MATERIALIZE_FIELDS, materialize_schema),
        )
        .bound(),
        RepositoryCommand::Export => {
            Declaration::named("export", "", Fields::new(EXPORT_FIELDS, export_schema))
        }
        RepositoryCommand::Import => {
            Declaration::named("import", "", Fields::new(IMPORT_FIELDS, import_schema)).bound()
        }
        // Git runs merge-jsonl as a merge driver.
        RepositoryCommand::MergeJsonl => Declaration::named(
            "merge-jsonl",
            "",
            Fields::new(MERGE_JSONL_FIELDS, merge_jsonl_schema).target(&["base", "ours", "theirs"]),
        )
        .internal(),
        RepositoryCommand::Validate => Declaration::named(
            "validate",
            "",
            Fields::new(VALIDATE_FIELDS, validate_schema).target(&["artifact"]),
        ),
        RepositoryCommand::SchemaShow => Declaration::named(
            "schema show",
            "",
            Fields::new(SCHEMA_SHOW_FIELDS, schema_show_schema).target(&["artifact"]),
        ),
        RepositoryCommand::DictionaryImport => Declaration::named(
            "dictionary import",
            "Import the ASD-STE100 Issue 9 dictionary from one local PDF file",
            Fields::new(DICTIONARY_IMPORT_FIELDS, dictionary_import_schema),
        )
        .bound(),
        RepositoryCommand::DictionaryStatus => Declaration::named(
            "dictionary status",
            "Report the committed dictionary reference and the analyzer it selects",
            Fields::new(DICTIONARY_STATUS_FIELDS, dictionary_status_schema),
        ),
    }
}
