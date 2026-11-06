//! Declarations of the commands that set up, export, import, merge, and check
//! repository files.

use super::Declaration;
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

pub(super) const fn declaration(command: RepositoryCommand) -> Declaration {
    match command {
        RepositoryCommand::Init => Declaration::named("init", "", init_schema),
        RepositoryCommand::CargoInit => Declaration::named("__cargo-init", "", cargo_init_schema),
        RepositoryCommand::Materialize => Declaration::named("materialize", "", materialize_schema),
        RepositoryCommand::Export => Declaration::named("export", "", export_schema),
        RepositoryCommand::Import => Declaration::named("import", "", import_schema),
        RepositoryCommand::MergeJsonl => Declaration::named("merge-jsonl", "", merge_jsonl_schema),
        RepositoryCommand::Validate => Declaration::named("validate", "", validate_schema),
        RepositoryCommand::SchemaShow => Declaration::named("schema show", "", schema_show_schema),
        RepositoryCommand::DictionaryImport => Declaration::named(
            "dictionary import",
            "Import the ASD-STE100 Issue 9 dictionary from one local PDF file",
            dictionary_import_schema,
        ),
        RepositoryCommand::DictionaryStatus => Declaration::named(
            "dictionary status",
            "Report the committed dictionary reference and the analyzer it selects",
            dictionary_status_schema,
        ),
    }
}
