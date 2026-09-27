use super::{OutputIdentity, OutputState, PublicationOutput, TransactionDirectory};
use crate::safe_fs::{ChildKind, Directory};
use crate::wiki::publish::PublishError;
use camino::Utf8Path;
use provenance_macros::rule;

/// Decides whether a publication may start against the path it was pointed at.
///
/// Publication replaces a whole directory tree, so it refuses to start from
/// any state it cannot reason about: a symlink at the output, which would let
/// a link redirect the replacement onto a tree the caller never named; a
/// non-directory at the output, which belongs to the caller; or any artifact
/// an interrupted earlier run left beside it (the lock, the lock cleanup, the
/// stage, the stage cleanup, or the backup).
///
/// Crash residue is reported by path and left exactly as found. Only an
/// operator can tell whether a leftover backup holds the live wiki or a
/// half-written one, so this code never adopts residue as its own work and
/// never deletes it to clear the way. The refusal is the whole response.
///
/// All five artifacts are probed before that refusal is built, so the operator
/// is told about everything an interrupted run left rather than about whichever
/// leftover happened to be looked at first. Clearing one artifact and retrying
/// only to be sent back for the next is the failure this avoids.
#[rule("rule_publish_preflight")]
pub(in crate::wiki::publish) fn preflight(
    output: &PublicationOutput,
    transaction: &TransactionDirectory,
) -> Result<OutputState, PublishError> {
    let paths = &transaction.paths;
    match transaction
        .parent
        .child_kind(&transaction.output_leaf)
        .map_err(|error| PublishError::io("inspect wiki output", &output.path, error))?
    {
        Some(ChildKind::Symlink) => {
            return Err(PublishError::OutputSymlink {
                path: output.path.clone(),
            })
        }
        Some(ChildKind::File | ChildKind::Other) => {
            return Err(PublishError::OutputNotDirectory {
                path: output.path.clone(),
            })
        }
        _ => {}
    }
    let output_state = transaction.output_identity(&transaction.output_leaf, &output.path)?;
    transaction.validate_output(&transaction.output_leaf, &output.path, output.policy)?;

    let mut ambiguous = Vec::new();
    let mut unsafe_lock = None;
    for (leaf, path, is_lock) in [
        (&transaction.leaves.lock, &paths.lock, true),
        (&transaction.leaves.lock_cleanup, &paths.lock_cleanup, false),
        (&transaction.leaves.stage, &paths.stage, false),
        (
            &transaction.leaves.stage_cleanup,
            &paths.stage_cleanup,
            false,
        ),
        (&transaction.leaves.backup, &paths.backup, false),
    ] {
        let Some(kind) = transaction
            .parent
            .child_kind(leaf)
            .map_err(|error| PublishError::io("inspect publication artifact", path, error))?
        else {
            continue;
        };
        if is_lock && kind != ChildKind::File {
            unsafe_lock = Some(path.clone());
        }
        ambiguous.push(path.clone());
    }
    if !ambiguous.is_empty() {
        return Err(PublishError::AmbiguousArtifacts {
            paths: ambiguous,
            unsafe_lock,
        });
    }
    Ok(output_state)
}

impl TransactionDirectory {
    pub(super) fn verify_requested_parent(&self, output: &Utf8Path) -> Result<(), PublishError> {
        let parent_path = output
            .parent()
            .filter(|path| !path.as_str().is_empty())
            .unwrap_or_else(|| Utf8Path::new("."));
        let expected = same_file::Handle::from_file(
            self.parent
                .try_clone()
                .map(Directory::into_file)
                .map_err(|error| {
                    PublishError::io("clone output parent handle", parent_path, error)
                })?,
        )
        .map_err(|error| PublishError::io("record output parent identity", parent_path, error))?;
        let current = Directory::open(parent_path.as_std_path(), "output parent")
            .map(Directory::into_file)
            .and_then(same_file::Handle::from_file)
            .map_err(|error| PublishError::OutputChanged {
                path: output.to_path_buf(),
                detail: format!("output parent cannot be verified: {error}"),
            })?;
        if current == expected {
            Ok(())
        } else {
            Err(PublishError::OutputChanged {
                path: output.to_path_buf(),
                detail: "output parent no longer matches the opened transaction directory"
                    .to_string(),
            })
        }
    }

    pub(super) fn output_identity(
        &self,
        leaf: &str,
        display: &Utf8Path,
    ) -> Result<OutputState, PublishError> {
        match self.open_dir(leaf) {
            Ok(file) => same_file::Handle::from_file(file)
                .map(OutputIdentity)
                .map(OutputState::Existing)
                .map_err(|error| PublishError::io("open wiki output identity", display, error)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(OutputState::Absent),
            Err(error) => Err(PublishError::io(
                "inspect wiki output identity",
                display,
                error,
            )),
        }
    }
}

#[cfg(test)]
pub(super) fn output_identity(output: &Utf8Path) -> Result<OutputState, PublishError> {
    match output.symlink_metadata() {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(OutputState::Absent)
        }
        Err(error) => {
            return Err(PublishError::io(
                "inspect wiki output identity",
                output,
                error,
            ))
        }
    }
    let identity = same_file::Handle::from_path(output.as_std_path())
        .map_err(|error| PublishError::io("open wiki output identity", output, error))?;
    Ok(OutputState::Existing(OutputIdentity(identity)))
}
