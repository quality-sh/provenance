use super::install_decision::{classify_install, InstallVerdict, TargetEntry, TargetState};
use super::{file_report, skill_name, EmbeddedSkill, FileInstallReport, FileStatus, InstallReport};
use crate::atomic_file::{atomic_replace, FileSnapshot};
use anyhow::Context;
use provenance_macros::rule;
use std::path::{Path, PathBuf};

mod auxiliary;
use auxiliary::{plan_agents_cleanup, plan_copy_files, RemoveAction};

pub(super) struct InstallPlan {
    global: bool,
    canonical_dir: PathBuf,
    claude_dir: PathBuf,
    canonical: Vec<FileAction>,
    claude: Vec<ClaudeAction>,
    legacy: Vec<RemoveAction>,
    agents: Option<FileAction>,
    link_mode: &'static str,
    fallback_reason: Option<String>,
}

#[derive(Clone, Copy)]
pub(super) enum InstallRequest {
    Init,
    Standalone {
        global: bool,
        force: bool,
        copy: bool,
    },
}

impl InstallPlan {
    pub(super) fn build(base: &Path, request: InstallRequest) -> anyhow::Result<Self> {
        let (global, force, copy, cleanup_agents) = match request {
            InstallRequest::Init => (false, false, false, false),
            InstallRequest::Standalone {
                global,
                force,
                copy,
            } => (global, force, copy, true),
        };
        let canonical_dir = base.join(".agents/skills");
        let claude_dir = base.join(".claude/skills");
        let mut canonical = Vec::new();
        for skill in super::EMBEDDED_SKILLS {
            canonical.push(FileAction::managed(
                canonical_dir.join(skill.directory).join("SKILL.md"),
                super::render::skill_file(skill).into_bytes(),
                force,
            )?);
        }

        let mut claude = Vec::new();
        let mut link_mode = if copy { "copy" } else { "symlink" };
        let mut fallback_reason = None;
        for skill in super::EMBEDDED_SKILLS {
            let action = ClaudeAction::plan(skill, &canonical_dir, &claude_dir, force, copy)?;
            if action.uses_copy_fallback() && !copy {
                link_mode = "copy-fallback";
                fallback_reason.get_or_insert_with(|| action.fallback_reason());
            }
            claude.push(action);
        }

        let mut legacy = Vec::new();
        for path in crate::legacy_cleanup::skill_paths(base) {
            if let Some(action) = RemoveAction::plan(path)? {
                legacy.push(action);
            }
        }
        let agents = cleanup_agents
            .then(|| plan_agents_cleanup(base, global))
            .transpose()?
            .flatten();
        Ok(Self {
            global,
            canonical_dir,
            claude_dir,
            canonical,
            claude,
            legacy,
            agents,
            link_mode,
            fallback_reason,
        })
    }

    pub(super) fn apply(mut self) -> anyhow::Result<InstallReport> {
        let mut files = Vec::new();
        for action in self.canonical {
            files.push(action.apply()?);
        }
        for action in self.claude {
            let (reports, runtime_fallback) = action.apply()?;
            if let Some(reason) = runtime_fallback {
                self.link_mode = "copy-fallback";
                self.fallback_reason.get_or_insert(reason);
            }
            files.extend(reports);
        }
        for action in self.legacy {
            files.push(action.apply()?);
        }
        if let Some(action) = self.agents {
            files.push(action.apply()?);
        }
        Ok(InstallReport {
            global: self.global,
            status: super::combined_status(&files),
            canonical_dir: self.canonical_dir.display().to_string(),
            claude_dir: self.claude_dir.display().to_string(),
            link_mode: self.link_mode,
            fallback_reason: self.fallback_reason,
            files,
        })
    }

    pub(super) fn recheck(&self) -> anyhow::Result<()> {
        for action in &self.canonical {
            action.recheck()?;
        }
        for action in &self.claude {
            action.recheck()?;
        }
        for action in &self.legacy {
            action.recheck()?;
        }
        if let Some(action) = &self.agents {
            action.recheck()?;
        }
        Ok(())
    }
}

pub(super) struct FileAction {
    path: PathBuf,
    before: FileSnapshot,
    contents: Vec<u8>,
    status: FileStatus,
    observed: bool,
}

impl FileAction {
    #[rule("rule_init_upgrades_hash_owned_skills")]
    pub(super) fn managed(path: PathBuf, contents: Vec<u8>, force: bool) -> anyhow::Result<Self> {
        let entry = TargetEntry::read(&path)?;
        let before = match entry {
            TargetEntry::Vacant => FileSnapshot::Missing,
            TargetEntry::Other => FileSnapshot::read(&path)?,
            _ => {
                let verdict = classify_install(TargetState::Foreign, force);
                if verdict == InstallVerdict::Refuse {
                    anyhow::bail!(
                        "{} exists and differs; rerun with --force to overwrite",
                        path.display()
                    );
                }
                anyhow::bail!("{} is not a regular file", path.display());
            }
        };
        let unchanged = before.bytes() == Some(contents.as_slice());
        let state = match before.bytes() {
            None => TargetState::Clear,
            Some(existing) if super::ownership::may_replace(existing, &contents) => {
                TargetState::Ours
            }
            Some(_) => TargetState::Foreign,
        };
        let verdict = classify_install(state, force);
        if verdict == InstallVerdict::Refuse {
            anyhow::bail!(
                "{} exists and differs; rerun with --force to overwrite",
                path.display()
            );
        }
        let status = if unchanged {
            FileStatus::Unchanged
        } else if matches!(before, FileSnapshot::Missing) {
            FileStatus::Installed
        } else {
            FileStatus::Updated
        };
        Ok(Self {
            path,
            before,
            contents,
            status,
            observed: true,
        })
    }

    pub(super) const fn exact(
        path: PathBuf,
        before: FileSnapshot,
        contents: Vec<u8>,
        status: FileStatus,
        observed: bool,
    ) -> Self {
        Self {
            path,
            before,
            contents,
            status,
            observed,
        }
    }

    fn recheck(&self) -> anyhow::Result<()> {
        if self.observed {
            self.before.recheck(&self.path)?;
        }
        Ok(())
    }

    fn apply(self) -> anyhow::Result<FileInstallReport> {
        if self.status != FileStatus::Unchanged {
            atomic_replace(&self.path, &self.before, &self.contents)?;
        }
        Ok(file_report(&self.path, self.status))
    }
}

enum ClaudeAction {
    Symlink {
        path: PathBuf,
        target: PathBuf,
        before: TargetEntry,
        verdict: InstallVerdict,
        fallback: Vec<FileAction>,
    },
    Copy {
        path: PathBuf,
        before: TargetEntry,
        verdict: InstallVerdict,
        files: Vec<FileAction>,
    },
}

impl ClaudeAction {
    fn plan(
        skill: &'static EmbeddedSkill,
        canonical_dir: &Path,
        claude_dir: &Path,
        force: bool,
        copy: bool,
    ) -> anyhow::Result<Self> {
        let name = skill_name(skill)?;
        let path = claude_dir.join(name);
        let target = super::relative_claude_target(name);
        let before = TargetEntry::read(&path)?;
        if copy {
            let state = match &before {
                TargetEntry::Vacant | TargetEntry::Directory => TargetState::Clear,
                TargetEntry::Symlink(current) if current == &target => TargetState::Ours,
                TargetEntry::Symlink(_) | TargetEntry::Other => TargetState::Foreign,
            };
            let verdict = classify_install(state, force);
            refuse_claude(&path, &before, verdict, true)?;
            let prospective_missing = !matches!(before, TargetEntry::Directory);
            let files = plan_copy_files(skill, canonical_dir, &path, force, prospective_missing)?;
            return Ok(Self::Copy {
                path,
                before,
                verdict,
                files,
            });
        }

        let state = match &before {
            TargetEntry::Vacant => TargetState::Clear,
            TargetEntry::Symlink(current) if current == &target => TargetState::Ours,
            TargetEntry::Directory => TargetState::ForeignDirectory,
            TargetEntry::Symlink(_) | TargetEntry::Other => TargetState::Foreign,
        };
        let verdict = classify_install(state, force);
        refuse_claude(&path, &before, verdict, false)?;
        let fallback = if verdict == InstallVerdict::Ours {
            Vec::new()
        } else {
            plan_copy_files(
                skill,
                canonical_dir,
                &path,
                force,
                !matches!(before, TargetEntry::Directory),
            )?
        };
        Ok(Self::Symlink {
            path,
            target,
            before,
            verdict,
            fallback,
        })
    }

    const fn uses_copy_fallback(&self) -> bool {
        matches!(
            self,
            Self::Symlink {
                verdict: InstallVerdict::CopyInto,
                ..
            }
        )
    }

    fn fallback_reason(&self) -> String {
        match self {
            Self::Symlink { path, .. } => {
                format!("{} already exists as a directory", path.display())
            }
            Self::Copy { .. } => String::new(),
        }
    }

    fn apply(self) -> anyhow::Result<(Vec<FileInstallReport>, Option<String>)> {
        self.apply_with(super::create_dir_symlink)
    }

    fn apply_with(
        self,
        create_symlink: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
    ) -> anyhow::Result<(Vec<FileInstallReport>, Option<String>)> {
        match self {
            Self::Copy {
                path,
                before,
                verdict,
                files,
            } => {
                before.recheck(&path)?;
                if matches!(verdict, InstallVerdict::Ours | InstallVerdict::Overwrite) {
                    before.remove(&path)?;
                }
                Ok((
                    files
                        .into_iter()
                        .map(FileAction::apply)
                        .collect::<anyhow::Result<Vec<_>>>()?,
                    None,
                ))
            }
            Self::Symlink {
                path,
                target,
                before,
                verdict,
                fallback,
            } => {
                if verdict == InstallVerdict::Ours {
                    return Ok((vec![file_report(&path, FileStatus::Unchanged)], None));
                }
                before.recheck(&path)?;
                if verdict == InstallVerdict::CopyInto {
                    return Ok((
                        fallback
                            .into_iter()
                            .map(FileAction::apply)
                            .collect::<anyhow::Result<Vec<_>>>()?,
                        None,
                    ));
                }
                if verdict == InstallVerdict::Overwrite {
                    before.remove(&path)?;
                }
                std::fs::create_dir_all(path.parent().unwrap_or_else(|| Path::new(".")))?;
                match create_symlink(&target, &path) {
                    Ok(()) => Ok((
                        vec![file_report(
                            &path,
                            if matches!(before, TargetEntry::Vacant) {
                                FileStatus::Linked
                            } else {
                                FileStatus::Updated
                            },
                        )],
                        None,
                    )),
                    Err(error) => {
                        let reason = format!("failed to symlink {}: {error}", path.display());
                        let reports = fallback
                            .into_iter()
                            .map(FileAction::apply)
                            .collect::<anyhow::Result<Vec<_>>>()
                            .with_context(|| {
                                format!("failed to copy after symlink error: {error}")
                            })?;
                        Ok((reports, Some(reason)))
                    }
                }
            }
        }
    }

    fn recheck(&self) -> anyhow::Result<()> {
        match self {
            Self::Symlink {
                path,
                before,
                fallback,
                ..
            } => {
                before.recheck(path)?;
                for action in fallback {
                    action.recheck()?;
                }
            }
            Self::Copy {
                path,
                before,
                files,
                ..
            } => {
                before.recheck(path)?;
                for action in files {
                    action.recheck()?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

fn refuse_claude(
    path: &Path,
    entry: &TargetEntry,
    verdict: InstallVerdict,
    copy: bool,
) -> anyhow::Result<()> {
    if verdict != InstallVerdict::Refuse {
        return Ok(());
    }
    match entry {
        TargetEntry::Symlink(current) => anyhow::bail!(
            "{} {} {}; rerun with --force to overwrite",
            path.display(),
            if copy { "is a symlink to" } else { "points at" },
            current.display()
        ),
        _ => anyhow::bail!(
            "{} exists and is not a skill directory; rerun with --force to overwrite",
            path.display()
        ),
    }
}
