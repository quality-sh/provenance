//! The CLI grammar of the commands that install skills and render reports.

use super::{json_format_flag, CliForm, Flag, FlagKind, REPO, SCOPE};
use provenance_porcelain::registry::WorkspaceCommand;

const JSON_FORMAT: Flag = json_format_flag(Some("json"));

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

pub(super) const fn form(command: WorkspaceCommand) -> CliForm {
    match command {
        WorkspaceCommand::SkillsList => CliForm::new(SKILLS_LIST_FLAGS),
        WorkspaceCommand::SkillsShow => CliForm::new(SKILLS_SHOW_FLAGS).positional(&["name"]),
        WorkspaceCommand::SkillsInstall => CliForm::new(SKILLS_INSTALL_FLAGS),
        WorkspaceCommand::ReportRender => CliForm::new(REPORT_RENDER_FLAGS),
        WorkspaceCommand::ReportBuild => CliForm::new(REPORT_BUILD_FLAGS),
    }
}
