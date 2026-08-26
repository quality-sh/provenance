use super::FileAction;
use crate::atomic_file::FileSnapshot;
use crate::skills::install_decision::{classify_install, TargetEntry, TargetState};
use crate::skills::{file_report, skill_name, EmbeddedSkill, FileInstallReport, FileStatus};
use std::path::{Path, PathBuf};

pub(super) fn plan_copy_files(
    skill: &'static EmbeddedSkill,
    canonical_dir: &Path,
    destination: &Path,
    force: bool,
    prospective_missing: bool,
) -> anyhow::Result<Vec<FileAction>> {
    let name = skill_name(skill)?;
    let mut sources = vec![(
        PathBuf::from("SKILL.md"),
        crate::skills::render::skill_file(skill).into_bytes(),
    )];
    crate::skills::copy_tree::collect_sidecars(
        &canonical_dir.join(name),
        Path::new(""),
        &mut sources,
    )?;
    sources
        .into_iter()
        .map(|(relative, contents)| {
            let path = destination.join(relative);
            if prospective_missing {
                if let Err(error) = TargetEntry::read(&path) {
                    let blocked_by_planned_parent_replacement = error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|error| error.kind() == std::io::ErrorKind::NotADirectory);
                    if !blocked_by_planned_parent_replacement {
                        return Err(error);
                    }
                }
                let _ = classify_install(TargetState::Clear, force);
                return Ok(FileAction::exact(
                    path,
                    FileSnapshot::Missing,
                    contents,
                    FileStatus::Installed,
                    false,
                ));
            }
            FileAction::managed(path, contents, force)
        })
        .collect()
}

pub(super) struct RemoveAction {
    pub(super) path: PathBuf,
    pub(super) before: FileSnapshot,
    parent: PathBuf,
    parent_entries: Vec<std::ffi::OsString>,
}

impl RemoveAction {
    pub(super) fn plan(path: PathBuf) -> anyhow::Result<Option<Self>> {
        let entry = TargetEntry::read(&path)?;
        let state = match entry {
            TargetEntry::Vacant => TargetState::Clear,
            TargetEntry::Other => {
                let before = FileSnapshot::read(&path)?;
                let managed = before
                    .bytes()
                    .and_then(|bytes| std::str::from_utf8(bytes).ok())
                    .is_some_and(crate::legacy_cleanup::valid_managed_skill);
                let state = if managed {
                    TargetState::Ours
                } else {
                    TargetState::Foreign
                };
                let _ = classify_install(state, false);
                let parent = path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf();
                let parent_entries = directory_entries(&parent)?;
                return Ok(managed.then_some(Self {
                    path,
                    before,
                    parent,
                    parent_entries,
                }));
            }
            TargetEntry::Directory | TargetEntry::Symlink(_) => TargetState::Foreign,
        };
        let _ = classify_install(state, false);
        Ok(None)
    }

    pub(super) fn apply(self) -> anyhow::Result<FileInstallReport> {
        self.recheck()?;
        std::fs::remove_file(&self.path)?;
        if self.parent_entries.len() == 1 {
            std::fs::remove_dir(&self.parent)?;
        }
        Ok(file_report(&self.path, FileStatus::Removed))
    }

    pub(super) fn recheck(&self) -> anyhow::Result<()> {
        self.before.recheck(&self.path)?;
        anyhow::ensure!(
            directory_entries(&self.parent)? == self.parent_entries,
            "{} changed after skill cleanup was planned; retry",
            self.parent.display()
        );
        Ok(())
    }
}

fn directory_entries(path: &Path) -> anyhow::Result<Vec<std::ffi::OsString>> {
    let mut entries = std::fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    Ok(entries)
}

pub(super) fn plan_agents_cleanup(base: &Path, global: bool) -> anyhow::Result<Option<FileAction>> {
    let path = crate::legacy_cleanup::agents_path(base, global);
    let _ = TargetEntry::read(&path)?;
    let before = FileSnapshot::read(&path)?;
    let Some(existing) = before.bytes() else {
        let _ = classify_install(TargetState::Clear, false);
        return Ok(None);
    };
    let updated = crate::legacy_cleanup::project_agents(existing);
    let changed = updated != existing;
    let _ = classify_install(
        if changed {
            TargetState::Ours
        } else {
            TargetState::Clear
        },
        false,
    );
    Ok(changed.then(|| FileAction::exact(path, before, updated, FileStatus::Updated, true)))
}
