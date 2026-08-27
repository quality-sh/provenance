//! The one place `provenance skills install` decides whether it may write
//! over what is already at a target path.
//!
//! The installer writes to three kinds of target - the canonical `SKILL.md`
//! files, the `.claude/skills` symlinks, and the copies that stand in for
//! those symlinks - and each reads the filesystem in its own vocabulary: a
//! file's bytes, a symlink's target, a directory's presence. They all ask
//! the same question, so they all ask it here.

use provenance_macros::rule;
use std::path::{Path, PathBuf};

/// What is already at an install target, in the decision's own vocabulary.
/// Each caller looks at the target in its own terms and maps what it sees
/// onto one of these four states before asking.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetState {
    /// Nothing at the target that the install would have to remove.
    Clear,
    /// Exactly what this install would put there: the same bytes, the same
    /// link target. Left alone or written again, the user loses nothing.
    Ours,
    /// Content the install did not write.
    Foreign,
    /// Content the install did not write, in a form the install can work
    /// through rather than over: a directory where a symlink was wanted,
    /// which the copy path fills in, leaving everything else under it.
    ForeignDirectory,
}

/// What the caller may do at the target.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InstallVerdict {
    /// Nothing is in the way: write.
    Write,
    /// The target already holds what this install would write. Leave it, or
    /// put the same content there in another form; either way nothing of the
    /// user's is lost.
    Ours,
    /// Clear the content and write over it: `--force` was given.
    Overwrite,
    /// Stop without writing. The caller words the refusal, naming what is in
    /// the way and saying that `--force` overwrites it.
    Refuse,
    /// Leave the directory standing and install into it by copying.
    CopyInto,
}

/// Whether an install may write over what is already at a target path.
///
/// What this protects is the user's own files at the target: a hand-written
/// `SKILL.md`, a symlink they pointed somewhere else, a directory they keep
/// under `.claude/skills`. The install may write where nothing is, and where
/// its own output already sits; anything else it did not write, it leaves
/// alone.
///
/// `--force` is the only override: no other flag, environment variable or
/// target path lets an install destroy a file it did not write, and an
/// install that refuses always says so rather than writing quietly. One case
/// is not an override but a gentler route: a directory where a symlink was
/// wanted is not destroyed at all, it is copied into, so whatever else lives
/// under it survives.
#[rule("rule_install_never_clobbers")]
pub const fn classify_install(state: TargetState, force: bool) -> InstallVerdict {
    match state {
        TargetState::Clear => InstallVerdict::Write,
        TargetState::Ours => InstallVerdict::Ours,
        TargetState::Foreign | TargetState::ForeignDirectory if force => InstallVerdict::Overwrite,
        TargetState::Foreign => InstallVerdict::Refuse,
        TargetState::ForeignDirectory => InstallVerdict::CopyInto,
    }
}

/// What is actually at a target path, read once. The callers keep it beside
/// the verdict so they can clear the target and word their own refusal; the
/// kinds are raw filesystem facts, and each caller maps them onto a
/// `TargetState` itself, because the same directory blocks a symlink but is
/// exactly what a copy wants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetEntry {
    /// Nothing is at the path.
    Vacant,
    /// A symlink, with the path it points at.
    Symlink(PathBuf),
    /// A directory.
    Directory,
    /// Anything else: a plain file where a skill directory belongs, a
    /// socket, a device node.
    Other,
}

impl TargetEntry {
    pub fn read(path: &Path) -> anyhow::Result<Self> {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Self::Vacant),
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink() {
            Ok(Self::Symlink(std::fs::read_link(path)?))
        } else if metadata.is_dir() {
            Ok(Self::Directory)
        } else {
            Ok(Self::Other)
        }
    }

    pub fn recheck(&self, path: &Path) -> anyhow::Result<()> {
        let current = Self::read(path)?;
        anyhow::ensure!(
            &current == self,
            "{} changed after installation was planned; retry",
            path.display()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use provenance_macros::verifies;

    use super::*;

    // Derived from an exhaustive match so that adding a `TargetState` fails
    // compilation until the new state joins the chain, keeping the proofs
    // below complete.
    fn all_states() -> Vec<TargetState> {
        let mut all = vec![TargetState::Clear];
        while let Some(next) = match all.last().unwrap() {
            TargetState::Clear => Some(TargetState::Ours),
            TargetState::Ours => Some(TargetState::Foreign),
            TargetState::Foreign => Some(TargetState::ForeignDirectory),
            TargetState::ForeignDirectory => None,
        } {
            all.push(next);
        }
        all
    }

    /// Independent restatement of what the rule protects, used as the oracle
    /// below: a target holds the user's own work when the install did not
    /// write what is there. Must not be implemented by calling
    /// `classify_install`.
    const fn holds_user_content(state: TargetState) -> bool {
        matches!(state, TargetState::Foreign | TargetState::ForeignDirectory)
    }

    /// The one verdict that destroys what is at the target.
    const fn destroys_target(verdict: InstallVerdict) -> bool {
        matches!(verdict, InstallVerdict::Overwrite)
    }

    /// The domain is finite and small: four states the callers can report,
    /// each with `--force` given or withheld, so the eight cases below are
    /// every call that can ever be made.
    #[test]
    #[verifies("rule_install_never_clobbers", exhaustion)]
    fn nothing_the_install_did_not_write_is_destroyed_without_force() {
        for state in all_states() {
            for force in [false, true] {
                let verdict = classify_install(state, force);
                if destroys_target(verdict) {
                    assert!(
                        force && holds_user_content(state),
                        "{state:?} with force={force} is overwritten"
                    );
                }
                if holds_user_content(state) && !force {
                    assert!(
                        !destroys_target(verdict),
                        "{state:?} loses the user's content without --force"
                    );
                }
            }
        }
    }

    /// `--force` is an override, not a lever: it only ever changes the answer
    /// where the user's own content is at stake, and an install never demands
    /// it for a target that holds nothing of theirs.
    #[test]
    #[verifies("rule_install_never_clobbers", exhaustion)]
    fn force_changes_nothing_where_the_user_has_nothing_at_stake() {
        for state in all_states() {
            if holds_user_content(state) {
                continue;
            }
            let withheld = classify_install(state, false);
            assert_eq!(
                withheld,
                classify_install(state, true),
                "--force changes the verdict for {state:?}"
            );
            assert_ne!(
                withheld,
                InstallVerdict::Refuse,
                "{state:?} demands --force although nothing of the user's is there"
            );
        }
    }
}
