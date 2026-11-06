//! Declarations of the CLI-only commands that serve docs and review pages, install skills,
//! and render reports.

use super::{
    json_format_flag, CliForm, Declaration, Flag, FlagKind, FlagValue, Input, REPO, SCOPE,
};
use serde::Deserialize;

/// One CLI-only workspace command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceCommand {
    DocsCheck,
    DocsServe,
    SkillsList,
    SkillsShow,
    SkillsInstall,
    ReportRender,
    ReportBuild,
    Review,
    Mcp,
}

const JSON_FORMAT: Flag = json_format_flag(Some("json"));

const DOCS_CHECK_FLAGS: &[Flag] = &[REPO, JSON_FORMAT];

const DOCS_SERVE_FLAGS: &[Flag] = &[
    REPO,
    Flag::new("host", FlagKind::Value).default("127.0.0.1"),
    Flag::new("port", FlagKind::Value)
        .value(FlagValue::Port)
        .default("5174"),
];

const SKILLS_LIST_FLAGS: &[Flag] = &[JSON_FORMAT];

const SKILLS_SHOW_FLAGS: &[Flag] = &[Flag::new("name", FlagKind::Positional).required()];

const SKILLS_INSTALL_FLAGS: &[Flag] = &[
    Flag::new("global", FlagKind::Switch),
    Flag::new("copy", FlagKind::Switch),
    Flag::new("force", FlagKind::Switch),
    JSON_FORMAT,
];

const REPORT_RENDER_FLAGS: &[Flag] = &[
    Flag::new("input", FlagKind::Value)
        .required()
        .help("Report envelope JSON (schema version 1)"),
    Flag::new("format", FlagKind::Value)
        .values(&["json", "markdown"])
        .default("markdown"),
    Flag::new("output", FlagKind::Value)
        .help("Write the result to this file instead of standard output"),
];

const REPORT_BUILD_FLAGS: &[Flag] = &[
    REPO,
    Flag::new("base", FlagKind::Value)
        .required()
        .help("Older endpoint of the comparison range"),
    Flag::new("head", FlagKind::Value)
        .required()
        .help("Newer endpoint of the comparison range"),
    Flag::new("repository", FlagKind::Value)
        .required()
        .help("Repository identity in `owner/name` form for report links"),
    SCOPE,
    Flag::new("path", FlagKind::Value).help(
        "Scan this path instead of the repository root; a partial scan is recorded as incomplete",
    ),
    Flag::new("output", FlagKind::Value)
        .help("Write the envelope JSON to this file instead of standard output"),
];

const REVIEW_FLAGS: &[Flag] = &[
    Flag::new("repo", FlagKind::Value)
        .required()
        .help("Repository with an initialized Provenance manifest"),
    Flag::new("repository-id", FlagKind::Value)
        .required()
        .help("Opaque repository target exposed to the browser"),
    Flag::new("scope", FlagKind::Value).required(),
    Flag::new("port", FlagKind::Value)
        .value(FlagValue::Port)
        .default("0")
        .help("Loopback port. Zero selects an available port"),
    Flag::new("no-open", FlagKind::Switch).help("Do not open the review page in a browser"),
    Flag::new("detach", FlagKind::Switch).help(
        "Reuse the running host for this repository and scope, or start one in the background \
         that keeps running after this command returns. Prints where the review page is served",
    ),
];

const MCP_FLAGS: &[Flag] = &[REPO, SCOPE];

/// The `docs check` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocsCheckInput {
    pub repo: String,
    pub format: String,
}

/// The `docs serve` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocsServeInput {
    pub repo: String,
    pub host: String,
    pub port: u16,
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

/// The `skills install` input. Each bool is one CLI switch.
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

/// The `review` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewInput {
    pub repo: String,
    pub repository_id: String,
    pub scope: String,
    pub port: u16,
    #[serde(default)]
    pub no_open: bool,
    #[serde(default)]
    pub detach: bool,
}

/// The `mcp` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct McpInput {
    pub repo: String,
    pub scope: String,
}

macro_rules! schema_fn {
    ($name:ident, $input:ty) => {
        fn $name() -> serde_json::Value {
            serde_json::to_value(schemars::schema_for!($input)).expect("schema is JSON")
        }
    };
}

schema_fn!(docs_check_schema, DocsCheckInput);
schema_fn!(docs_serve_schema, DocsServeInput);
schema_fn!(skills_list_schema, SkillsListInput);
schema_fn!(skills_show_schema, SkillsShowInput);
schema_fn!(skills_install_schema, SkillsInstallInput);
schema_fn!(report_render_schema, ReportRenderInput);
schema_fn!(report_build_schema, ReportBuildInput);
schema_fn!(review_schema, ReviewInput);
schema_fn!(mcp_schema, McpInput);

/// A shown CLI-only command without flag relations.
const fn cli_only(
    name: &'static str,
    words: &'static [&'static str],
    description: &'static str,
    schema: fn() -> serde_json::Value,
    flags: &'static [Flag],
) -> Declaration {
    Declaration {
        name,
        description,
        cli_form: CliForm::Words(words),
        cli_input: Input::Typed(schema),
        mcp_input: Input::None,
        positional: &[],
        relations: &[],
        hidden: false,
        flags,
    }
}

pub(super) fn declaration(command: WorkspaceCommand) -> Declaration {
    match command {
        WorkspaceCommand::DocsCheck => cli_only(
            "docs check",
            &["docs", "check"],
            "",
            docs_check_schema,
            DOCS_CHECK_FLAGS,
        ),
        WorkspaceCommand::DocsServe => cli_only(
            "docs serve",
            &["docs", "serve"],
            "",
            docs_serve_schema,
            DOCS_SERVE_FLAGS,
        ),
        WorkspaceCommand::SkillsList => cli_only(
            "skills list",
            &["skills", "list"],
            "",
            skills_list_schema,
            SKILLS_LIST_FLAGS,
        ),
        WorkspaceCommand::SkillsShow => Declaration {
            positional: &["name"],
            ..cli_only(
                "skills show",
                &["skills", "show"],
                "",
                skills_show_schema,
                SKILLS_SHOW_FLAGS,
            )
        },
        WorkspaceCommand::SkillsInstall => cli_only(
            "skills install",
            &["skills", "install"],
            "",
            skills_install_schema,
            SKILLS_INSTALL_FLAGS,
        ),
        WorkspaceCommand::ReportRender => cli_only(
            "report render",
            &["report", "render"],
            "Render the deterministic pull request report from a report envelope",
            report_render_schema,
            REPORT_RENDER_FLAGS,
        ),
        WorkspaceCommand::ReportBuild => cli_only(
            "report build",
            &["report", "build"],
            "Build a report envelope from a real repository at a named base and head",
            report_build_schema,
            REPORT_BUILD_FLAGS,
        ),
        WorkspaceCommand::Review => cli_only(
            "review",
            &["review"],
            "Serve local review assets and authorized repository operations",
            review_schema,
            REVIEW_FLAGS,
        ),
        WorkspaceCommand::Mcp => cli_only(
            "mcp",
            &["mcp"],
            "Serve the MCP tools for one repository on standard input and output",
            mcp_schema,
            MCP_FLAGS,
        ),
    }
}
