use super::{Access, StateStore};
use crate::layout::ProvenanceLayout;
use crate::publication::{with_repository_publication, PublicationGuard};
use camino::Utf8Path;
use provenance_macros::rule;

#[cfg(test)]
mod tests;

impl Clone for StateStore {
    fn clone(&self) -> Self {
        Self::new(self.layout.clone())
    }
}

impl StateStore {
    /// Reads the guarded repository without a second lock.
    pub(crate) fn under_guard(guard: &PublicationGuard) -> Self {
        Self {
            layout: guard.layout().clone(),
            access: Access::Held,
        }
    }

    pub(super) fn state_path_access<R>(
        &self,
        path: &Utf8Path,
        operation: impl FnOnce() -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        crate::test_probes::record_read(path);
        let Some(state_dir) = path.ancestors().find(|ancestor| {
            ancestor.file_name() == Some("state")
                && ancestor.parent().and_then(Utf8Path::file_name) == Some(".provenance")
        }) else {
            return operation();
        };
        let root = state_dir
            .parent()
            .and_then(Utf8Path::parent)
            .ok_or_else(|| anyhow::anyhow!("state path has no repository root"))?;
        self.publication_access(&ProvenanceLayout::new(root), operation)
    }

    /// Uses the publication lock for writes.
    pub fn with_repository_publication<R>(
        &self,
        operation: impl FnOnce() -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        with_repository_publication(&self.layout, operation)
    }

    pub(super) fn with_repository_read<R>(
        &self,
        operation: impl FnOnce() -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        self.publication_access(&self.layout, operation)
    }

    /// A store under a held publication guard reads without a second lock.
    #[rule("rule_store_under_guard_takes_no_second_lock")]
    fn publication_access<R>(
        &self,
        layout: &ProvenanceLayout,
        operation: impl FnOnce() -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        match self.access {
            Access::Lock => with_repository_publication(layout, operation),
            Access::Held => operation(),
        }
    }
}
