use super::{manifest, PublicationOutput, PublishError};
use camino::{Utf8Path, Utf8PathBuf};
use std::fs::File;

mod cleanup;
mod ownership;
mod parent_walk;
mod replacement;

pub(super) use ownership::open_child_directory_no_follow;
pub(super) use ownership::{acquire_lock, preflight};
// Stage identity reads through a path on Windows only; the test helper uses
// it on every platform.
#[cfg(any(windows, test))]
pub(super) use ownership::open_directory_no_follow;
pub(super) use replacement::replace_output;
#[cfg(test)]
pub(super) use replacement::replace_output_with;

pub(super) enum OutputState {
    Absent,
    Existing(OutputIdentity),
}

pub(super) struct OutputIdentity(same_file::Handle);

pub(super) struct StageIdentity(same_file::Handle);

impl StageIdentity {
    pub(super) fn from_file(file: &File) -> std::io::Result<Self> {
        same_file::Handle::from_file(file.try_clone()?).map(Self)
    }
}

pub(super) struct PublicationLock {
    file: File,
    identity: same_file::Handle,
}

pub(super) struct TransactionDirectory {
    parent: File,
    pub paths: TransactionPaths,
    output_leaf: String,
    leaves: ArtifactLeaves,
}

impl TransactionDirectory {
    pub(super) fn open(output: &Utf8Path) -> Result<Self, PublishError> {
        let paths = TransactionPaths::new(output)?;
        let parent_path = output
            .parent()
            .filter(|path| !path.as_str().is_empty())
            .unwrap_or_else(|| Utf8Path::new("."));
        let parent = parent_walk::open_or_create_parent(parent_path, output)?;
        let output_leaf = output
            .file_name()
            .expect("validated output leaf")
            .to_string();
        Ok(Self {
            parent,
            paths,
            leaves: ArtifactLeaves::beside(&output_leaf),
            output_leaf,
        })
    }

    pub(super) fn create_stage(&self) -> std::io::Result<File> {
        // mkdir_at hardcodes its access mask (no read-attributes right on
        // Windows), which is why the stage identity comes from a separate
        // no-follow reopen rather than this handle.
        fs_at::OpenOptions::default().mkdir_at(&self.parent, &self.leaves.stage)
    }

    fn create_file(&self, leaf: &str) -> std::io::Result<File> {
        let mut options = fs_at::OpenOptions::default();
        // Read access too: the lock handle's identity is read back via
        // GetFileInformationByHandle on Windows, which a write-only handle
        // refuses with ERROR_ACCESS_DENIED.
        options
            .read(true)
            .write(fs_at::OpenOptionsWriteMode::Write)
            .create_new(true)
            .follow(false);
        options.open_at(&self.parent, leaf)
    }

    fn open_dir(&self, leaf: &str) -> std::io::Result<File> {
        ownership::open_child_directory_no_follow(&self.parent, leaf)
    }

    fn child_identity(&self, leaf: &str) -> std::io::Result<same_file::Handle> {
        let file = if leaf == self.leaves.lock || leaf == self.leaves.lock_cleanup {
            let mut options = fs_at::OpenOptions::default();
            options.read(true).follow(false);
            options.open_at(&self.parent, leaf)?
        } else {
            self.open_dir(leaf)?
        };
        same_file::Handle::from_file(file)
    }

    fn rename(&self, from: &str, to: &str) -> std::io::Result<()> {
        replacement::rename_no_replace_at(&self.parent, self.parent_path(), from, to)
    }

    /// The directory every transaction artifact sits in, as a path.
    fn parent_path(&self) -> &Utf8Path {
        self.paths
            .lock
            .parent()
            .expect("transaction artifacts have a parent directory")
    }

    fn remove_file(&self, leaf: &str) -> std::io::Result<()> {
        fs_at::OpenOptions::default().unlink_at(&self.parent, leaf)
    }

    pub(super) fn validate_output(
        &self,
        leaf: &str,
        display: &Utf8Path,
        policy: super::OutputPolicy,
    ) -> Result<(), PublishError> {
        let directory = match self.open_dir(leaf) {
            Ok(directory) => Some(directory),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(PublishError::io("open wiki output", display, error)),
        };
        manifest::validate_output_handle(directory, display, policy)
    }
}

/// The five leaf names an interrupted publication can leave beside an output.
///
/// This is the only place the names are built. Both the handle-based
/// transaction directory and the display paths it reports come from here, so
/// the names the publisher opens and the names it prints cannot drift apart.
/// A transaction directory keeps one of these, so the five names are carried,
/// passed and reasoned about as the one set they are.
pub(super) struct ArtifactLeaves {
    pub lock: String,
    pub lock_cleanup: String,
    pub stage: String,
    pub stage_cleanup: String,
    pub backup: String,
}

impl ArtifactLeaves {
    /// Mints the artifact leaf names that sit beside an output named
    /// `output_leaf`, one per role, all hidden and all carrying the output's
    /// own name so two outputs in one directory never collide.
    pub(super) fn beside(output_leaf: &str) -> Self {
        let leaf = |role: &str| format!(".{output_leaf}.provenance-wiki.{role}");
        Self {
            lock: leaf("lock"),
            lock_cleanup: leaf("lock.cleanup"),
            stage: leaf("stage"),
            stage_cleanup: leaf("stage.cleanup"),
            backup: leaf("backup"),
        }
    }
}

pub(super) struct TransactionPaths {
    pub lock: Utf8PathBuf,
    pub lock_cleanup: Utf8PathBuf,
    pub stage: Utf8PathBuf,
    pub stage_cleanup: Utf8PathBuf,
    pub backup: Utf8PathBuf,
}

impl TransactionPaths {
    pub(super) fn new(output: &Utf8Path) -> Result<Self, PublishError> {
        let parent = output
            .parent()
            .filter(|path| !path.as_str().is_empty())
            .unwrap_or_else(|| Utf8Path::new("."));
        let leaf = output
            .file_name()
            .ok_or_else(|| PublishError::InvalidOutputPath {
                path: output.to_path_buf(),
                detail: "path has no file name".to_string(),
            })?;
        let leaves = ArtifactLeaves::beside(leaf);
        Ok(Self {
            lock: parent.join(leaves.lock),
            lock_cleanup: parent.join(leaves.lock_cleanup),
            stage: parent.join(leaves.stage),
            stage_cleanup: parent.join(leaves.stage_cleanup),
            backup: parent.join(leaves.backup),
        })
    }
}

#[cfg(test)]
mod tests;
