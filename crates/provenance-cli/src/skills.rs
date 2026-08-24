mod copy_tree;
mod install_decision;
mod render;
pub mod stamp;

use anyhow::Context;
use install_decision::{classify_install, InstallVerdict, TargetEntry, TargetState};
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
        content: include_str!("../../../skills/provenance-fork-tournament/SKILL.md"),
    },
    EmbeddedSkill {
        directory: "provenance-grounded-writing",
        content: include_str!("../../../skills/provenance-grounded-writing/SKILL.md"),
    },
    EmbeddedSkill {
        directory: "provenance-shaping",
        content: include_str!("../../../skills/provenance-shaping/SKILL.md"),
    },
    EmbeddedSkill {
        directory: "provenance-swarm-backtrace",
        content: include_str!("../../../skills/provenance-swarm-backtrace/SKILL.md"),
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

enum ClaudeInstall {
    Symlink(FileInstallReport),
    CopyFallback {
        reports: Vec<FileInstallReport>,
        reason: String,
    },
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

fn install_at(base: &Path, global: bool, force: bool, copy: bool) -> anyhow::Result<InstallReport> {
    let canonical_dir = base.join(".agents/skills");
    let claude_dir = base.join(".claude/skills");
    let mut files = install_canonical_skill_files(&canonical_dir, force)?;
    let mut link_mode = if copy { "copy" } else { "symlink" };
    let mut fallback_reason = None;

    for skill in EMBEDDED_SKILLS {
        if copy {
            files.extend(copy_skill_dir(skill, &canonical_dir, &claude_dir, force)?);
            continue;
        }

        match install_claude_symlink_or_copy(skill, &canonical_dir, &claude_dir, force)? {
            ClaudeInstall::Symlink(report) => files.push(report),
            ClaudeInstall::CopyFallback { reports, reason } => {
                link_mode = "copy-fallback";
                fallback_reason.get_or_insert(reason);
                files.extend(reports);
            }
        }
    }
    files.extend(
        crate::legacy_cleanup::cleanup(base, global)?
            .into_iter()
            .map(|change| file_report(&change.path, change.status)),
    );

    Ok(InstallReport {
        global,
        status: combined_status(&files),
        canonical_dir: canonical_dir.display().to_string(),
        claude_dir: claude_dir.display().to_string(),
        link_mode,
        fallback_reason,
        files,
    })
}

fn install_canonical_skill_files(
    skills_dir: &Path,
    force: bool,
) -> anyhow::Result<Vec<FileInstallReport>> {
    EMBEDDED_SKILLS
        .iter()
        .map(|skill| {
            let path = skills_dir.join(skill.directory).join("SKILL.md");
            write_managed_file(&path, &render::skill_file(skill), force)
        })
        .collect()
}

fn install_claude_symlink_or_copy(
    skill: &EmbeddedSkill,
    canonical_dir: &Path,
    claude_dir: &Path,
    force: bool,
) -> anyhow::Result<ClaudeInstall> {
    let name = skill_name(skill)?;
    let link_path = claude_dir.join(name);
    let target = relative_claude_target(name);
    let entry = TargetEntry::read(&link_path)?;
    // A symlink already pointing at the canonical directory is what this
    // install writes; a directory can be copied into instead of removed.
    let state = match &entry {
        TargetEntry::Vacant => TargetState::Clear,
        TargetEntry::Symlink(current) if current == &target => TargetState::Ours,
        TargetEntry::Symlink(_) | TargetEntry::Other => TargetState::Foreign,
        TargetEntry::Directory => TargetState::ForeignDirectory,
    };

    let status = match classify_install(state, force) {
        InstallVerdict::Write => FileStatus::Linked,
        InstallVerdict::Ours => {
            return Ok(ClaudeInstall::Symlink(file_report(
                &link_path,
                FileStatus::Unchanged,
            )))
        }
        InstallVerdict::CopyInto => {
            let reason = format!("{} already exists as a directory", link_path.display());
            return Ok(ClaudeInstall::CopyFallback {
                reports: copy_skill_dir(skill, canonical_dir, claude_dir, force)
                    .with_context(|| reason.clone())?,
                reason,
            });
        }
        InstallVerdict::Refuse => match &entry {
            TargetEntry::Symlink(current) => anyhow::bail!(
                "{} points at {}; rerun with --force to overwrite",
                link_path.display(),
                current.display()
            ),
            _ => anyhow::bail!(
                "{} exists and is not a skill directory; rerun with --force to overwrite",
                link_path.display()
            ),
        },
        InstallVerdict::Overwrite => {
            entry.remove(&link_path)?;
            FileStatus::Updated
        }
    };

    create_symlink_or_copy(
        skill,
        canonical_dir,
        claude_dir,
        &link_path,
        &target,
        status,
    )
}

fn create_symlink_or_copy(
    skill: &EmbeddedSkill,
    canonical_dir: &Path,
    claude_dir: &Path,
    link_path: &Path,
    target: &Path,
    status: FileStatus,
) -> anyhow::Result<ClaudeInstall> {
    if let Some(parent) = link_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match create_dir_symlink(target, link_path) {
        Ok(()) => Ok(ClaudeInstall::Symlink(file_report(link_path, status))),
        // Callers only reach this with link_path absent or already cleared,
        // so the copy needs no overwrite permission: force stays false.
        Err(error) => Ok(ClaudeInstall::CopyFallback {
            reports: copy_skill_dir(skill, canonical_dir, claude_dir, false)
                .with_context(|| format!("failed to copy after symlink error: {error}"))?,
            reason: format!("failed to symlink {}: {error}", link_path.display()),
        }),
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

/// Copies one skill from the canonical directory to `.claude/skills`. The
/// copy holds the full skill directory, not only its `SKILL.md`.
fn copy_skill_dir(
    skill: &EmbeddedSkill,
    canonical_dir: &Path,
    claude_dir: &Path,
    force: bool,
) -> anyhow::Result<Vec<FileInstallReport>> {
    let name = skill_name(skill)?;
    let destination = claude_dir.join(name);
    let entry = TargetEntry::read(&destination)?;
    // A copy wants a directory here, so an existing one is the container it
    // fills in rather than something in its way. Swapping our own canonical
    // symlink for a copy keeps identical content, so it needs no --force;
    // any other symlink is foreign.
    let state = match &entry {
        TargetEntry::Vacant | TargetEntry::Directory => TargetState::Clear,
        TargetEntry::Symlink(current) if current == &relative_claude_target(name) => {
            TargetState::Ours
        }
        TargetEntry::Symlink(_) | TargetEntry::Other => TargetState::Foreign,
    };
    match classify_install(state, force) {
        // Copying is itself the route a diverted install takes, so there is
        // nowhere gentler to send it.
        InstallVerdict::Write | InstallVerdict::CopyInto => {}
        InstallVerdict::Ours | InstallVerdict::Overwrite => entry.remove(&destination)?,
        InstallVerdict::Refuse => match &entry {
            TargetEntry::Symlink(current) => anyhow::bail!(
                "{} is a symlink to {}; rerun with --force to replace it with a copy",
                destination.display(),
                current.display()
            ),
            _ => anyhow::bail!(
                "{} exists and is not a skill directory; rerun with --force to overwrite",
                destination.display()
            ),
        },
    }

    copy_tree::copy_tree(&canonical_dir.join(name), &destination, force)
}

fn write_managed_file(
    path: &Path,
    contents: &str,
    force: bool,
) -> anyhow::Result<FileInstallReport> {
    write_managed_bytes(path, contents.as_bytes(), force)
}

/// Writes `contents` at `path` under the installer's ownership rules. The
/// bytes form also serves the files that a skill directory carries beside its
/// `SKILL.md`, such as a picture below `assets/`.
fn write_managed_bytes(
    path: &Path,
    contents: &[u8],
    force: bool,
) -> anyhow::Result<FileInstallReport> {
    let existing = if path.exists() {
        Some(std::fs::read(path)?)
    } else {
        None
    };
    let state = match &existing {
        None => TargetState::Clear,
        Some(current) if current == contents => TargetState::Ours,
        Some(_) => TargetState::Foreign,
    };
    match classify_install(state, force) {
        InstallVerdict::Ours => return Ok(file_report(path, FileStatus::Unchanged)),
        InstallVerdict::Refuse => anyhow::bail!(
            "{} exists and differs; rerun with --force to overwrite",
            path.display()
        ),
        InstallVerdict::Overwrite => {
            std::fs::write(path, contents)?;
            return Ok(file_report(path, FileStatus::Updated));
        }
        // A managed file is never a directory, so no install is ever
        // diverted here: write.
        InstallVerdict::Write | InstallVerdict::CopyInto => {}
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)?;
    Ok(file_report(path, FileStatus::Installed))
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
