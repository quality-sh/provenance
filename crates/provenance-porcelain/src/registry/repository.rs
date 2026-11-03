//! Declarations of the CLI-only commands that set up, export, import, merge, and check
//! repository files.

use super::{json_format_flag, CliForm, Declaration, Flag, FlagKind, Input, Relation, REPO, SCOPE};
use serde::Deserialize;

/// One CLI-only repository command.
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

/// The `__cargo-init` input that `cargo provenance init` passes on.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CargoInitInput {
    pub package: Option<String>,
    pub ste_pdf: Option<String>,
}

/// The `materialize` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterializeInput {
    pub repo: String,
    pub format: String,
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

/// The `import` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportInput {
    pub repo: String,
    pub scope: String,
    pub input: String,
    #[serde(default)]
    pub dry_run: bool,
    pub format: String,
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

/// The `dictionary import` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DictionaryImportInput {
    pub pdf: String,
    pub repo: String,
    pub format: String,
}

/// The `dictionary status` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DictionaryStatusInput {
    pub repo: String,
    pub format: String,
}

macro_rules! schema_fn {
    ($name:ident, $input:ty) => {
        fn $name() -> serde_json::Value {
            serde_json::to_value(schemars::schema_for!($input)).expect("schema is JSON")
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

/// A shown CLI-only command without an about line or flag relations.
const fn cli_only(
    name: &'static str,
    words: &'static [&'static str],
    schema: fn() -> serde_json::Value,
    flags: &'static [Flag],
) -> Declaration {
    Declaration {
        name,
        description: "",
        cli_form: CliForm::Words(words),
        cli_input: Input::Typed(schema),
        mcp_input: Input::None,
        positional: &[],
        relations: &[],
        hidden: false,
        flags,
    }
}

pub(super) fn declaration(command: RepositoryCommand) -> Declaration {
    match command {
        RepositoryCommand::Init => Declaration {
            relations: &[
                Relation::Requires("path_prefix", "scope"),
                Relation::Conflicts("clear_disposition_actors", "disposition_actor_id"),
            ],
            ..cli_only("init", &["init"], init_schema, INIT_FLAGS)
        },
        RepositoryCommand::CargoInit => Declaration {
            hidden: true,
            ..cli_only(
                "__cargo-init",
                &["__cargo-init"],
                cargo_init_schema,
                CARGO_INIT_FLAGS,
            )
        },
        RepositoryCommand::Materialize => cli_only(
            "materialize",
            &["materialize"],
            materialize_schema,
            MATERIALIZE_FLAGS,
        ),
        RepositoryCommand::Export => cli_only("export", &["export"], export_schema, EXPORT_FLAGS),
        RepositoryCommand::Import => cli_only("import", &["import"], import_schema, IMPORT_FLAGS),
        RepositoryCommand::MergeJsonl => Declaration {
            positional: &["base", "ours", "theirs"],
            ..cli_only(
                "merge-jsonl",
                &["merge-jsonl"],
                merge_jsonl_schema,
                MERGE_JSONL_FLAGS,
            )
        },
        RepositoryCommand::Validate => Declaration {
            positional: &["artifact"],
            ..cli_only("validate", &["validate"], validate_schema, VALIDATE_FLAGS)
        },
        RepositoryCommand::SchemaShow => Declaration {
            positional: &["artifact"],
            ..cli_only(
                "schema show",
                &["schema", "show"],
                schema_show_schema,
                SCHEMA_SHOW_FLAGS,
            )
        },
        RepositoryCommand::DictionaryImport => Declaration {
            description: "Import the ASD-STE100 Issue 9 dictionary from one local PDF file",
            ..cli_only(
                "dictionary import",
                &["dictionary", "import"],
                dictionary_import_schema,
                DICTIONARY_IMPORT_FLAGS,
            )
        },
        RepositoryCommand::DictionaryStatus => Declaration {
            description: "Report the committed dictionary reference and the analyzer it selects",
            ..cli_only(
                "dictionary status",
                &["dictionary", "status"],
                dictionary_status_schema,
                DICTIONARY_STATUS_FLAGS,
            )
        },
    }
}
