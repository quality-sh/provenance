//! Declarations of the commands that check docs, install skills, and render reports.

use super::Declaration;
use serde::Deserialize;

/// One workspace command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceCommand {
    DocsCheck,
    SkillsList,
    SkillsShow,
    SkillsInstall,
    ReportRender,
    ReportBuild,
}

/// The `docs check` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocsCheckInput {
    pub repo: String,
    pub format: String,
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

schema_fn!(docs_check_schema, DocsCheckInput);
schema_fn!(skills_list_schema, SkillsListInput);
schema_fn!(skills_show_schema, SkillsShowInput);
schema_fn!(skills_install_schema, SkillsInstallInput);
schema_fn!(report_render_schema, ReportRenderInput);
schema_fn!(report_build_schema, ReportBuildInput);

pub(super) const fn declaration(command: WorkspaceCommand) -> Declaration {
    match command {
        WorkspaceCommand::DocsCheck => Declaration::named("docs check", "", docs_check_schema),
        WorkspaceCommand::SkillsList => Declaration::named("skills list", "", skills_list_schema),
        WorkspaceCommand::SkillsShow => Declaration::named("skills show", "", skills_show_schema),
        WorkspaceCommand::SkillsInstall => {
            Declaration::named("skills install", "", skills_install_schema)
        }
        WorkspaceCommand::ReportRender => Declaration::named(
            "report render",
            "Render the deterministic pull request report from a report envelope",
            report_render_schema,
        ),
        WorkspaceCommand::ReportBuild => Declaration::named(
            "report build",
            "Build a report envelope from a real repository at a named base and head",
            report_build_schema,
        ),
    }
}
