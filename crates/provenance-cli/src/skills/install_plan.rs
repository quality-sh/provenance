use super::install_decision::{classify_install, InstallVerdict, TargetEntry, TargetState};
use super::{file_report, FileInstallReport, FileStatus, InstallReport};
use crate::atomic_file::{ensure_managed_directory, FileRollbackJournal, FileSnapshot};
use provenance_macros::rule;
use std::path::{Path, PathBuf};

mod auxiliary;
use auxiliary::{plan_agents_cleanup, RemoveAction};
mod claude;
use claude::ClaudeAction;

pub(super) struct InstallPlan {
    base: PathBuf,
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
        ensure_managed_directory(base, &canonical_dir)?;
        ensure_managed_directory(base, &claude_dir)?;
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
            base: base.to_path_buf(),
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

    pub(super) fn apply(self) -> anyhow::Result<InstallReport> {
        let mut rollback = FileRollbackJournal::within(&self.base);
        match self.apply_in(&mut rollback) {
            Ok(report) => {
                rollback.commit()?;
                Ok(report)
            }
            Err(error) => match rollback.rollback() {
                Ok(()) => Err(error),
                Err(rollback) => Err(error.context(format!("skill rollback failed: {rollback:#}"))),
            },
        }
    }

    #[rule("rule_init_installs_bundled_skills")]
    pub(super) fn apply_in(
        mut self,
        rollback: &mut FileRollbackJournal,
    ) -> anyhow::Result<InstallReport> {
        ensure_managed_directory(&self.base, &self.canonical_dir)?;
        ensure_managed_directory(&self.base, &self.claude_dir)?;
        let mut files = Vec::new();
        for action in self.canonical {
            files.push(action.apply(rollback)?);
        }
        for action in self.claude {
            let (reports, runtime_fallback) = action.apply(rollback)?;
            if let Some(reason) = runtime_fallback {
                self.link_mode = "copy-fallback";
                self.fallback_reason.get_or_insert(reason);
            }
            files.extend(reports);
        }
        for action in self.legacy {
            files.push(action.apply(rollback)?);
        }
        if let Some(action) = self.agents {
            files.push(action.apply(rollback)?);
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

    fn apply(self, rollback: &mut FileRollbackJournal) -> anyhow::Result<FileInstallReport> {
        if self.status != FileStatus::Unchanged {
            rollback.replace(&self.path, &self.before, &self.contents)?;
        }
        Ok(file_report(&self.path, self.status))
    }
}

#[cfg(test)]
mod tests;
