use anyhow::Context;
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

#[derive(Serialize)]
pub struct InstallReport {
    global: bool,
    status: &'static str,
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
    status: &'static str,
}

enum ClaudeInstall {
    Symlink(FileInstallReport),
    CopyFallback {
        report: FileInstallReport,
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
            files.push(copy_skill_dir(skill, &canonical_dir, &claude_dir, force)?);
            continue;
        }

        match install_claude_symlink_or_copy(skill, &canonical_dir, &claude_dir, force)? {
            ClaudeInstall::Symlink(report) => files.push(report),
            ClaudeInstall::CopyFallback { report, reason } => {
                link_mode = "copy-fallback";
                fallback_reason.get_or_insert(reason);
                files.push(report);
            }
        }
    }

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
            write_managed_file(&path, &render_skill_file(skill), force)
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

    let status = match std::fs::symlink_metadata(&link_path) {
        Err(_) => "linked",
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                let current_target = std::fs::read_link(&link_path)?;
                if current_target == target {
                    return Ok(ClaudeInstall::Symlink(file_report(&link_path, "unchanged")));
                }
                anyhow::ensure!(
                    force,
                    "{} points at {}; rerun with --force to overwrite",
                    link_path.display(),
                    current_target.display()
                );
                std::fs::remove_file(&link_path)?;
            } else if metadata.is_dir() {
                if !force {
                    let reason = format!("{} already exists as a directory", link_path.display());
                    return Ok(ClaudeInstall::CopyFallback {
                        report: copy_skill_dir(skill, canonical_dir, claude_dir, force)
                            .with_context(|| reason.clone())?,
                        reason,
                    });
                }
                std::fs::remove_dir_all(&link_path)?;
            } else {
                anyhow::ensure!(
                    force,
                    "{} exists and is not a skill directory; rerun with --force to overwrite",
                    link_path.display()
                );
                std::fs::remove_file(&link_path)?;
            }
            "updated"
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
    status: &'static str,
) -> anyhow::Result<ClaudeInstall> {
    if let Some(parent) = link_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match create_dir_symlink(target, link_path) {
        Ok(()) => Ok(ClaudeInstall::Symlink(file_report(link_path, status))),
        // Callers only reach this with link_path absent or already cleared,
        // so the copy needs no overwrite permission: force stays false.
        Err(error) => Ok(ClaudeInstall::CopyFallback {
            report: copy_skill_dir(skill, canonical_dir, claude_dir, false)
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

fn copy_skill_dir(
    skill: &EmbeddedSkill,
    canonical_dir: &Path,
    claude_dir: &Path,
    force: bool,
) -> anyhow::Result<FileInstallReport> {
    let name = skill_name(skill)?;
    let destination = claude_dir.join(name);
    if let Ok(metadata) = std::fs::symlink_metadata(&destination) {
        if metadata.file_type().is_symlink() {
            // Replacing our own canonical symlink with a copy keeps identical
            // content, so it needs no --force; anything else is foreign.
            let current_target = std::fs::read_link(&destination)?;
            anyhow::ensure!(
                force || current_target == relative_claude_target(name),
                "{} is a symlink to {}; rerun with --force to replace it with a copy",
                destination.display(),
                current_target.display()
            );
            std::fs::remove_file(&destination)?;
        } else if !metadata.is_dir() {
            anyhow::ensure!(
                force,
                "{} exists and is not a skill directory; rerun with --force to overwrite",
                destination.display()
            );
            std::fs::remove_file(&destination)?;
        }
    }

    let source_file = canonical_dir.join(name).join("SKILL.md");
    let contents = std::fs::read_to_string(&source_file)?;
    write_managed_file(&destination.join("SKILL.md"), &contents, force)
}

fn write_managed_file(
    path: &Path,
    contents: &str,
    force: bool,
) -> anyhow::Result<FileInstallReport> {
    if path.exists() {
        let current = std::fs::read_to_string(path)?;
        if current == contents {
            return Ok(file_report(path, "unchanged"));
        }
        anyhow::ensure!(
            force,
            "{} exists and differs; rerun with --force to overwrite",
            path.display()
        );
        std::fs::write(path, contents)?;
        return Ok(file_report(path, "updated"));
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)?;
    Ok(file_report(path, "installed"))
}

fn render_skill_file(skill: &EmbeddedSkill) -> String {
    const FRONTMATTER_START: &str = "---\n";
    const FRONTMATTER_END: &str = "\n---\n";

    let header = provenance_header(skill.content);
    if let Some(rest) = skill.content.strip_prefix(FRONTMATTER_START) {
        if let Some(end) = rest.find(FRONTMATTER_END) {
            let insertion = FRONTMATTER_START.len() + end + FRONTMATTER_END.len();
            return format!(
                "{}{}\n{}",
                &skill.content[..insertion],
                header,
                &skill.content[insertion..]
            );
        }
    }
    format!("{header}\n{}", skill.content)
}

fn provenance_header(content: &str) -> String {
    format!(
        "<!-- Installed by provenance {}; content hash fnv1a64:{} -->",
        env!("CARGO_PKG_VERSION"),
        fnv1a64(content)
    )
}

fn combined_status(files: &[FileInstallReport]) -> &'static str {
    if files.iter().any(|file| file.status == "updated") {
        "updated"
    } else if files
        .iter()
        .any(|file| matches!(file.status, "installed" | "linked"))
    {
        "installed"
    } else {
        "unchanged"
    }
}

fn file_report(path: &Path, status: &'static str) -> FileInstallReport {
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

fn frontmatter_field<'a>(content: &'a str, field: &str) -> Option<&'a str> {
    let mut lines = content.lines();
    if lines.next()? != "---" {
        return None;
    }
    let prefix = format!("{field}:");
    for line in lines {
        if line == "---" {
            return None;
        }
        if let Some(value) = line.strip_prefix(&prefix) {
            return Some(value.trim());
        }
    }
    None
}

fn fnv1a64(content: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in content.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn home_dir() -> anyhow::Result<PathBuf> {
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
mod tests {
    use super::*;

    #[test]
    fn home_dir_uses_userprofile_when_home_is_absent() {
        let resolved = home_dir_from_env(|key| {
            if key == "USERPROFILE" {
                Some(OsString::from(r"C:\Users\Ada"))
            } else {
                None
            }
        })
        .unwrap();

        assert_eq!(resolved, PathBuf::from(r"C:\Users\Ada"));
    }
}
