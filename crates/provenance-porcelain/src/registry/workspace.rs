//! Declarations of the commands that install skills and render reports.

use super::{json_format, Declaration, Field, Fields, REPO, SCOPE};
use serde::Deserialize;

/// One workspace command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceCommand {
    SkillsList,
    SkillsShow,
    SkillsInstall,
    ReportRender,
    ReportBuild,
}

/// The `skills list` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillsListInput {
    pub format: String,
}

/// The `skills show` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillsShowInput {
    pub name: String,
}

/// The `skills install` input. Each bool is one switch.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillsInstallInput {
    #[serde(default)]
    pub global: bool,
    #[serde(default)]
    pub copy: bool,
    #[serde(default)]
    pub force: bool,
    pub format: String,
}

/// The `report render` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportRenderInput {
    pub input: String,
    pub format: String,
    pub output: Option<String>,
}

/// The `report build` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportBuildInput {
    pub repo: String,
    pub base: String,
    pub head: String,
    pub repository: String,
    pub scope: String,
    pub path: Option<String>,
    pub output: Option<String>,
}

macro_rules! schema_fn {
    ($name:ident, $input:ty) => {
        fn $name() -> serde_json::Value {
            serde_json::to_value(schemars::schema_for!($input)).expect("schema is JSON")
        }
    };
}

schema_fn!(skills_list_schema, SkillsListInput);
schema_fn!(skills_show_schema, SkillsShowInput);
schema_fn!(skills_install_schema, SkillsInstallInput);
schema_fn!(report_render_schema, ReportRenderInput);
schema_fn!(report_build_schema, ReportBuildInput);

const JSON_FORMAT: Field = json_format(Some("json"));

const SKILLS_LIST_FIELDS: &[Field] = &[JSON_FORMAT];

const SKILLS_SHOW_FIELDS: &[Field] = &[Field::text("name").required()];

const SKILLS_INSTALL_FIELDS: &[Field] = &[
    Field::switch("global"),
    Field::switch("copy"),
    Field::switch("force"),
    JSON_FORMAT,
];

const REPORT_RENDER_FIELDS: &[Field] = &[
    Field::text("input")
        .required()
        .doc("Report envelope JSON (schema version 1)"),
    Field::text("format")
        .values(&["json", "markdown"])
        .default("markdown"),
    Field::text("output").doc("Write the result to this file instead of standard output"),
];

const REPORT_BUILD_FIELDS: &[Field] = &[
    REPO,
    Field::text("base")
        .required()
        .doc("Older endpoint of the comparison range"),
    Field::text("head")
        .required()
        .doc("Newer endpoint of the comparison range"),
    Field::text("repository")
        .required()
        .doc("Repository identity in `owner/name` form for report links"),
    SCOPE,
    Field::text("path").doc(
        "Scan this path instead of the repository root; a partial scan is recorded as incomplete",
    ),
    Field::text("output").doc("Write the envelope JSON to this file instead of standard output"),
];

pub(super) const fn declaration(command: WorkspaceCommand) -> Declaration {
    match command {
        WorkspaceCommand::SkillsList => Declaration::named(
            "skills list",
            "",
            Fields::new(SKILLS_LIST_FIELDS, skills_list_schema),
        ),
        WorkspaceCommand::SkillsShow => Declaration::named(
            "skills show",
            "",
            Fields::new(SKILLS_SHOW_FIELDS, skills_show_schema).target(&["name"]),
        ),
        WorkspaceCommand::SkillsInstall => Declaration::named(
            "skills install",
            "",
            Fields::new(SKILLS_INSTALL_FIELDS, skills_install_schema),
        ),
        WorkspaceCommand::ReportRender => Declaration::named(
            "report render",
            "Render the deterministic pull request report from a report envelope",
            Fields::new(REPORT_RENDER_FIELDS, report_render_schema),
        ),
        WorkspaceCommand::ReportBuild => Declaration::named(
            "report build",
            "Build a report envelope from a real repository at a named base and head",
            Fields::new(REPORT_BUILD_FIELDS, report_build_schema),
        ),
    }
}
