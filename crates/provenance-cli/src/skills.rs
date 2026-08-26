mod copy_tree;
mod install_decision;
mod install_plan;
mod ownership;
mod render;
pub mod stamp;

use anyhow::Context;
use provenance_macros::rule;
use render::frontmatter_field;
use serde::Serialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const INSTALL_COMMAND: &str = "provenance skills install";

struct EmbeddedSkill {
    directory: &'static str,
    content: &'static str,
}

const EMBEDDED_SKILLS: &[EmbeddedSkill] = &[
    EmbeddedSkill {
        directory: "provenance-fork-tournament",
        content: include_str!("../skills/provenance-fork-tournament/SKILL.md"),
    },
    EmbeddedSkill {
        directory: "provenance-grounded-writing",
        content: include_str!("../skills/provenance-grounded-writing/SKILL.md"),
    },
    EmbeddedSkill {
        directory: "provenance-shaping",
        content: include_str!("../skills/provenance-shaping/SKILL.md"),
    },
    EmbeddedSkill {
        directory: "provenance-swarm-backtrace",
        content: include_str!("../skills/provenance-swarm-backtrace/SKILL.md"),
    },
];

#[derive(Serialize)]
pub struct SkillSummary {
    name: String,
    description: String,
}

/// What an install run did to one file. These five outcomes are the whole
/// vocabulary of an install report, written both here and by
/// `legacy_cleanup`, and read back by `combined_status` to describe the run.
/// The serialised names are the strings the JSON report has always carried,
/// so the wire format does not depend on the Rust spelling.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    /// Already byte-for-byte what provenance would write.
    Unchanged,
    /// Newly written where nothing was.
    Installed,
    /// Rewritten over something else.
    Updated,
    /// A new symlink into the canonical directory.
    Linked,
    /// A legacy file deleted by cleanup.
    Removed,
}

#[derive(Serialize)]
pub struct InstallReport {
    global: bool,
    status: FileStatus,
    canonical_dir: String,
    claude_dir: String,
    link_mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallback_reason: Option<String>,
    files: Vec<FileInstallReport>,
}

#[derive(Serialize)]
pub struct SkillInstallStatus {
    pub installed: bool,
    pub install_command: &'static str,
    pub missing_skills: Vec<String>,
}

#[derive(Serialize)]
struct FileInstallReport {
    path: String,
    status: FileStatus,
}

pub fn list() -> anyhow::Result<Vec<SkillSummary>> {
    let mut summaries = EMBEDDED_SKILLS
        .iter()
        .map(|skill| {
            Ok(SkillSummary {
                name: skill_name(skill)?.to_string(),
                description: frontmatter_field(skill.content, "description")
                    .context("embedded skill is missing description")?
                    .to_string(),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    summaries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(summaries)
}

pub fn show(name: &str) -> anyhow::Result<&'static str> {
    for skill in EMBEDDED_SKILLS {
        if skill_name(skill)? == name {
            return Ok(skill.content);
        }
    }
    anyhow::bail!("unknown skill: {name}")
}

pub fn install(global: bool, force: bool, copy: bool) -> anyhow::Result<InstallReport> {
    let base = if global {
        home_dir()?
    } else {
        std::env::current_dir()?
    };
    install_at(&base, global, force, copy)
}

pub fn install_status(repo: &Path) -> anyhow::Result<SkillInstallStatus> {
    let mut missing_skills = Vec::new();
    for skill in EMBEDDED_SKILLS {
        if !canonical_skill_file(repo, skill).exists() {
            missing_skills.push(skill_name(skill)?.to_string());
        }
    }

    Ok(SkillInstallStatus {
        installed: missing_skills.is_empty(),
        install_command: INSTALL_COMMAND,
        missing_skills,
    })
}

pub fn render_status_markdown(status: &SkillInstallStatus) -> String {
    let installed = if status.installed { "yes" } else { "no" };
    format!(
        "\n## Skills\n- Installed: {installed}\n- Install command: `{}` from the repo root\n",
        status.install_command
    )
}

pub fn install_at(
    base: &Path,
    global: bool,
    force: bool,
    copy: bool,
) -> anyhow::Result<InstallReport> {
    install_plan::InstallPlan::build(
        base,
        install_plan::InstallRequest::Standalone {
            global,
            force,
            copy,
        },
    )?
    .apply()
}

pub struct InitSkillPlan(install_plan::InstallPlan);

pub fn plan_init_at(base: &Path) -> anyhow::Result<InitSkillPlan> {
    Ok(InitSkillPlan(install_plan::InstallPlan::build(
        base,
        install_plan::InstallRequest::Init,
    )?))
}

impl InitSkillPlan {
    pub(crate) fn recheck(&self) -> anyhow::Result<()> {
        self.0.recheck()
    }

    pub(crate) fn apply_in(
        self,
        rollback: &mut crate::atomic_file::FileRollbackJournal,
    ) -> anyhow::Result<()> {
        self.0.apply_in(rollback).map(|_| ())
    }
}

#[cfg(unix)]
fn create_dir_symlink(target: &Path, link_path: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link_path)
}

#[cfg(windows)]
fn create_dir_symlink(target: &Path, link_path: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link_path)
}

/// An install run reports the strongest change any one of its files
/// underwent: "unchanged", "installed" or "updated". A file rewritten or
/// deleted makes the whole run "updated"; failing that, a file newly written
/// or newly linked makes it "installed"; only when every file already matched
/// what provenance would write is the run "unchanged". Order and repetition
/// carry no weight.
///
/// Nothing branches on the answer, but it is the one line a reader takes as
/// the account of the run, so it must never claim less than happened (a
/// rewrite reported as "unchanged") or more (a no-op reported as "updated").
#[rule("rule_install_run_status")]
fn combined_status(files: &[FileInstallReport]) -> FileStatus {
    if files
        .iter()
        .any(|file| matches!(file.status, FileStatus::Updated | FileStatus::Removed))
    {
        FileStatus::Updated
    } else if files
        .iter()
        .any(|file| matches!(file.status, FileStatus::Installed | FileStatus::Linked))
    {
        FileStatus::Installed
    } else {
        FileStatus::Unchanged
    }
}

fn file_report(path: &Path, status: FileStatus) -> FileInstallReport {
    FileInstallReport {
        path: path.display().to_string(),
        status,
    }
}

fn canonical_skill_file(repo: &Path, skill: &EmbeddedSkill) -> PathBuf {
    repo.join(".agents/skills")
        .join(skill.directory)
        .join("SKILL.md")
}

fn relative_claude_target(name: &str) -> PathBuf {
    PathBuf::from("..")
        .join("..")
        .join(".agents")
        .join("skills")
        .join(name)
}

fn skill_name(skill: &EmbeddedSkill) -> anyhow::Result<&'static str> {
    let name =
        frontmatter_field(skill.content, "name").context("embedded skill is missing name")?;
    anyhow::ensure!(
        name == skill.directory,
        "embedded skill name {name} does not match directory {}",
        skill.directory
    );
    Ok(name)
}

fn home_dir() -> anyhow::Result<PathBuf> {
    home_dir_from_env(|key| std::env::var_os(key))
}

fn home_dir_from_env(mut var: impl FnMut(&str) -> Option<OsString>) -> anyhow::Result<PathBuf> {
    if let Some(home) = var("HOME") {
        return Ok(PathBuf::from(home));
    }
    if let Some(profile) = var("USERPROFILE") {
        return Ok(PathBuf::from(profile));
    }
    if let (Some(mut drive), Some(path)) = (var("HOMEDRIVE"), var("HOMEPATH")) {
        drive.push(path);
        return Ok(PathBuf::from(drive));
    }
    anyhow::bail!("HOME or USERPROFILE is not set")
}

#[cfg(test)]
mod tests;
