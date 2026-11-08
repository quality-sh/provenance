//! The working-tree file steps that land with a whole set on local disk,
//! such as the replacement of a Source file in a source edit.

use super::whole_set::WholeSetHook;
use super::LocalStateStorage;
use crate::storage::{Atomicity, PublishFailure, Revision, WriteSet};

/// The working-tree file steps that land with a whole set, such as the
/// replacement of a Source file in a source edit.
pub struct FileInstall<'a>(pub &'a mut dyn WholeSetHook);

impl LocalStateStorage {
    /// Lands the whole set `writes` and the file steps of `file` in one
    /// transaction, with the marker and recovery of those steps.
    pub fn publish_with_file(
        &self,
        writes: &WriteSet,
        file: FileInstall<'_>,
    ) -> Result<Revision, PublishFailure> {
        if writes.atomicity() != Atomicity::WholeSet {
            return Err(PublishFailure::Refused(anyhow::anyhow!(
                "a file install lands only with a whole set"
            )));
        }
        let FileInstall(hook) = file;
        self.publish_with(writes, hook)
    }
}
