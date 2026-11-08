//! Staged publication shared by import and review saves: a stage collects
//! its writes on an overlay and publishes them as one whole set.
use super::{enter, held_here, refuse_if_held, HolderKind};
use crate::layout::ProvenanceLayout;
use crate::state_store::{RecordStamp, StateStore};
use crate::storage::{Atomicity, StateView, WriteSet};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// A stage body tried to enter a live publication section of its repository.
#[derive(Debug, thiserror::Error)]
#[error("a staged write cannot read or write the live state of {repository}")]
pub struct LiveAccessDuringStage {
    pub repository: Utf8PathBuf,
}

/// Runs a stage body. While it runs, a live publication section of the same
/// repository on this thread refuses, so the body cannot read or write the
/// live state through a captured repository, an open or a guard.
#[rule("rule_staged_store_lives_only_inside_stage")]
pub fn with_stage_body<R>(
    live: &ProvenanceLayout,
    body: impl FnOnce() -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    let _body = enter(
        super::publication_lock_identity(live)?,
        HolderKind::StageBody,
    );
    body()
}

/// Refuses live access to `live` through a store that holds its section, or
/// through a read-only store, while a stage body of that repository runs on
/// this thread.
pub fn refuse_held_access_during_stage(live: &ProvenanceLayout) -> anyhow::Result<()> {
    if !held_here(None, HolderKind::StageBody) {
        return Ok(());
    }
    refuse_during_stage(&super::publication_lock_identity(live)?, live.root())
}

pub(super) fn refuse_during_stage(key: &str, repository: &Utf8Path) -> anyhow::Result<()> {
    if held_here(Some(key), HolderKind::StageBody) {
        return Err(LiveAccessDuringStage {
            repository: repository.to_owned(),
        }
        .into());
    }
    Ok(())
}

/// One stage: an overlay on the view that the stage started from, and the
/// lock that its sections take.
///
/// The staged sections of the stage run one at a time. A panic in a staged
/// section poisons the lock, and the stage then refuses to publish. Each
/// write of the stage lands on the overlay; the live state does not change
/// until the stage publishes the difference as one whole set.
#[derive(Debug)]
pub struct StageLock {
    identity: String,
    root: Utf8PathBuf,
    sections: Mutex<()>,
    base: StateView,
    overlay: Mutex<StateView>,
    /// A test stage over a staged state root reads and writes its files.
    files: bool,
}

impl StageLock {
    /// A stage of the repository at `root` over `base`.
    pub(crate) fn new(root: &Utf8Path, base: StateView) -> Arc<Self> {
        static STAGES: AtomicU64 = AtomicU64::new(0);
        Arc::new(Self {
            identity: format!("stage {} of {root}", STAGES.fetch_add(1, Ordering::Relaxed)),
            root: root.to_owned(),
            sections: Mutex::new(()),
            overlay: Mutex::new(base.clone()),
            base,
            files: false,
        })
    }

    /// A test stage that reads and writes the files of the staged state root
    /// at `root` instead of an overlay.
    #[cfg(test)]
    pub(crate) fn over_files(root: &Utf8Path) -> Arc<Self> {
        Arc::new(Self {
            identity: root.to_string(),
            root: root.to_owned(),
            sections: Mutex::new(()),
            base: StateView::new(std::collections::BTreeMap::new()),
            overlay: Mutex::new(StateView::new(std::collections::BTreeMap::new())),
            files: true,
        })
    }

    /// Whether this is a test stage that reads and writes files.
    pub(crate) const fn reads_files(&self) -> bool {
        self.files
    }

    /// Runs one staged section. Another thread waits; a second entry on this
    /// thread returns [`PublicationHeld`](super::PublicationHeld).
    #[rule("rule_staged_sections_run_one_at_a_time")]
    pub(crate) fn run<R>(
        &self,
        operation: impl FnOnce() -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        refuse_if_held(&self.identity, HolderKind::Stage, &self.root)?;
        let _section = self
            .sections
            .lock()
            .map_err(|_| anyhow::anyhow!("a staged section of {} panicked", self.root))?;
        let _holder = enter(self.identity.clone(), HolderKind::Stage);
        operation()
    }

    /// Refuses when a staged section panicked.
    pub(crate) fn ensure_intact(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.sections.is_poisoned(),
            "a staged section of {} panicked; the stage is not published",
            self.root
        );
        Ok(())
    }

    /// Runs `read` on the base view with the writes of the stage applied.
    pub(crate) fn read<R>(&self, read: impl FnOnce(&StateView) -> R) -> R {
        read(&self.overlay())
    }

    /// Lands `writes` on the overlay.
    pub(crate) fn apply(&self, writes: &WriteSet) {
        let mut overlay = self.overlay();
        let mut entries = overlay.entries().clone();
        writes.apply_to(&mut entries);
        *overlay = StateView::new(entries);
    }

    /// Every change of the stage as one whole set on the revision where the
    /// stage started: a put for each key that is new or has other bytes, and
    /// a delete for each key of the base that the overlay does not hold.
    #[rule("rule_stage_lands_as_one_whole_set")]
    pub(crate) fn changes(&self) -> WriteSet {
        let overlay = self.overlay();
        let mut writes = WriteSet::new(Atomicity::WholeSet).on_base(self.base.revision().clone());
        for (key, bytes) in overlay.entries() {
            if self.base.get(key) != Some(bytes.as_slice()) {
                writes = writes.put(key.clone(), bytes.clone());
            }
        }
        for key in self.base.entries().keys() {
            if overlay.get(key).is_none() {
                writes = writes.delete(key.clone());
            }
        }
        writes
    }

    fn overlay(&self) -> std::sync::MutexGuard<'_, StateView> {
        // Every change replaces the whole view, so a poisoned overlay is intact.
        self.overlay.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Runs `prepare` on a stage over the state that `held` reads: the live
/// section, or the staged section of an outer stage. The stage body borrows
/// a staged store with the identity of `held` and records that carry
/// `stamp`. While the body runs, live access to the repository on this
/// thread refuses. A dry run drops the stage. Otherwise the changes land as
/// one whole set: through the adapter for the live section, or on the
/// overlay of the outer stage.
pub fn with_staged_state<R>(
    held: &StateStore,
    dry_run: bool,
    stamp: RecordStamp,
    prepare: impl FnOnce(&StateStore) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    let staged = held.begin_stage(stamp)?;
    let result = with_stage_body(&held.layout, || prepare(&staged))?;
    staged.ensure_stage_intact()?;
    if dry_run {
        return Ok(result);
    }
    held.land_stage(staged.stage_changes()?)?;
    Ok(result)
}

#[cfg(test)]
pub(super) use crate::storage::local::copy_directory;
