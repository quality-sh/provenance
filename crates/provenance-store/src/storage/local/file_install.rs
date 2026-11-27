//! The working-tree file steps that land with a whole set on local disk,
//! such as the replacement of a Source file in a source edit.

use super::whole_set::WholeSetHook;
use super::LocalStateStorage;
use crate::storage::{Atomicity, PublishFailure, WriteSet};

/// The working-tree file steps that land with a whole set, such as the
/// replacement of a Source file in a source edit.
pub struct FileInstall<'a>(pub &'a mut dyn WholeSetHook);

impl LocalStateStorage {
    /// Lands the whole set `writes` and the file steps of `file` in one
    /// transaction, with the marker and recovery of those steps.
    pub(crate) fn publish_with_file(
        &self,
        writes: &WriteSet,
        file: FileInstall<'_>,
    ) -> Result<(), PublishFailure> {
        if writes.atomicity() != Atomicity::WholeSet {
            return Err(PublishFailure::Refused(crate::refusal::fault!(
                "a file install lands only with a whole set"
            )));
        }
        let section =
            super::section::section_writes(&self.layout).map_err(PublishFailure::Refused)?;
        let _writes = section.as_deref().map(super::section::take);
        self.ensure_publishable(writes, section.is_some())?;
        let FileInstall(hook) = file;
        self.land_whole_set(writes, hook)
    }
}
