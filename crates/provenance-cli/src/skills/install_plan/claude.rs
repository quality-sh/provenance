use super::{auxiliary::plan_copy_files, FileAction};
use crate::atomic_file::FileRollbackJournal;
use crate::skills::install_decision::{classify_install, InstallVerdict, TargetEntry, TargetState};
use crate::skills::{file_report, skill_name, EmbeddedSkill, FileInstallReport, FileStatus};
use anyhow::Context;
use std::path::{Path, PathBuf};

pub(super) enum ClaudeAction {
    Symlink {
        path: PathBuf,
        target: PathBuf,
        before: TargetEntry,
        before_fingerprint: TargetFingerprint,
        verdict: InstallVerdict,
        fallback: Vec<FileAction>,
    },
    Copy {
        path: PathBuf,
        before: TargetEntry,
        before_fingerprint: TargetFingerprint,
        verdict: InstallVerdict,
        files: Vec<FileAction>,
    },
}

impl ClaudeAction {
    pub(super) fn plan(
        skill: &'static EmbeddedSkill,
        canonical_dir: &Path,
        claude_dir: &Path,
        force: bool,
        copy: bool,
    ) -> anyhow::Result<Self> {
        let name = skill_name(skill)?;
        let path = claude_dir.join(name);
        let target = crate::skills::relative_claude_target(name);
        let before = TargetEntry::read(&path)?;
        let before_fingerprint = TargetFingerprint::read(&path)?;
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
                before_fingerprint,
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
            before_fingerprint,
            verdict,
            fallback,
        })
    }

    pub(super) const fn uses_copy_fallback(&self) -> bool {
        matches!(
            self,
            Self::Symlink {
                verdict: InstallVerdict::CopyInto,
                ..
            }
        )
    }

    pub(super) fn fallback_reason(&self) -> String {
        match self {
            Self::Symlink { path, .. } => {
                format!("{} already exists as a directory", path.display())
            }
            Self::Copy { .. } => String::new(),
        }
    }

    pub(super) fn apply(
        self,
        rollback: &mut FileRollbackJournal,
    ) -> anyhow::Result<(Vec<FileInstallReport>, Option<String>)> {
        self.apply_with(crate::skills::create_dir_symlink, rollback)
    }

    pub(super) fn apply_with(
        self,
        create_symlink: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
        rollback: &mut FileRollbackJournal,
    ) -> anyhow::Result<(Vec<FileInstallReport>, Option<String>)> {
        match self {
            Self::Copy {
                path,
                before,
                before_fingerprint,
                verdict,
                files,
            } => {
                before.recheck(&path)?;
                if matches!(verdict, InstallVerdict::Ours | InstallVerdict::Overwrite) {
                    rollback
                        .displace_checked(&path, |backup| before_fingerprint.recheck(backup))?;
                }
                Ok((
                    files
                        .into_iter()
                        .map(|action| action.apply(rollback))
                        .collect::<anyhow::Result<Vec<_>>>()?,
                    None,
                ))
            }
            Self::Symlink {
                path,
                target,
                before,
                before_fingerprint,
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
                            .map(|action| action.apply(rollback))
                            .collect::<anyhow::Result<Vec<_>>>()?,
                        None,
                    ));
                }
                if verdict == InstallVerdict::Overwrite {
                    rollback
                        .displace_checked(&path, |backup| before_fingerprint.recheck(backup))?;
                }
                rollback.create_dir_all(path.parent().unwrap_or_else(|| Path::new(".")))?;
                match create_symlink(&target, &path) {
                    Ok(()) => {
                        rollback.record_created_symlink(&path, &target);
                        Ok((
                            vec![file_report(
                                &path,
                                if matches!(before, TargetEntry::Vacant) {
                                    FileStatus::Linked
                                } else {
                                    FileStatus::Updated
                                },
                            )],
                            None,
                        ))
                    }
                    Err(error) => {
                        let reason = format!("failed to symlink {}: {error}", path.display());
                        let reports = fallback
                            .into_iter()
                            .map(|action| action.apply(rollback))
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

    pub(super) fn recheck(&self) -> anyhow::Result<()> {
        match self {
            Self::Symlink {
                path,
                before,
                before_fingerprint,
                fallback,
                ..
            } => {
                before.recheck(path)?;
                before_fingerprint.recheck(path)?;
                for action in fallback {
                    action.recheck()?;
                }
            }
            Self::Copy {
                path,
                before,
                before_fingerprint,
                files,
                ..
            } => {
                before.recheck(path)?;
                before_fingerprint.recheck(path)?;
                for action in files {
                    action.recheck()?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum TargetFingerprint {
    Vacant,
    Entries(Vec<FingerprintEntry>),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct FingerprintEntry {
    relative: PathBuf,
    kind: FingerprintKind,
    permissions: u32,
}

#[derive(Debug, PartialEq, Eq)]
enum FingerprintKind {
    Directory,
    File(Vec<u8>),
    Symlink(PathBuf),
    Other(u64),
}

impl TargetFingerprint {
    fn read(path: &Path) -> anyhow::Result<Self> {
        match std::fs::symlink_metadata(path) {
            Ok(_) => {
                let mut entries = Vec::new();
                fingerprint_entry(path, path, &mut entries)?;
                Ok(Self::Entries(entries))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::Vacant),
            Err(error) => Err(error.into()),
        }
    }

    fn recheck(&self, path: &Path) -> anyhow::Result<()> {
        let current = Self::read(path)?;
        anyhow::ensure!(&current == self, "target contents changed after planning");
        Ok(())
    }
}

fn fingerprint_entry(
    root: &Path,
    path: &Path,
    entries: &mut Vec<FingerprintEntry>,
) -> anyhow::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    let file_type = metadata.file_type();
    let kind = if file_type.is_symlink() {
        FingerprintKind::Symlink(std::fs::read_link(path)?)
    } else if metadata.is_dir() {
        FingerprintKind::Directory
    } else if metadata.is_file() {
        FingerprintKind::File(std::fs::read(path)?)
    } else {
        FingerprintKind::Other(metadata.len())
    };
    entries.push(FingerprintEntry {
        relative: path.strip_prefix(root).unwrap_or(path).to_path_buf(),
        kind,
        permissions: permission_bits(&metadata.permissions()),
    });
    if metadata.is_dir() && !file_type.is_symlink() {
        let mut children = std::fs::read_dir(path)?
            .map(|entry| entry.map(|item| item.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        children.sort();
        for child in children {
            fingerprint_entry(root, &child, entries)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn permission_bits(permissions: &std::fs::Permissions) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    permissions.mode()
}

#[cfg(not(unix))]
fn permission_bits(permissions: &std::fs::Permissions) -> u32 {
    u32::from(permissions.readonly())
}

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
