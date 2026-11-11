//! The view that a store reads, and the read fence of a family reader.
//!
//! A catch-up parses a family again only when a file in one of its input
//! directories changed. So the family reader gets a [`FencedStore`]: a store
//! over the view of the pass whose reads are fenced to those directories.
//! The reader has no other store. A read outside the directories fails the
//! parse, also when the reader drops the error.

use super::access::GuardedStore;
use crate::storage::StateView;
use camino::{Utf8Path, Utf8PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

/// What a view store reads: one view, the parses of that view that the
/// store shares, and the fence of a fenced store.
#[derive(Debug)]
pub(super) struct ViewReads {
    pub view: Arc<StateView>,
    pub parses: Option<Arc<super::SharedParses>>,
    pub fence: Option<ReadFence>,
}

/// The directories below one scope directory that a fenced store reads.
#[derive(Debug)]
pub(super) struct ReadFence {
    scope_dir: Utf8PathBuf,
    directories: Vec<&'static str>,
    outside: Mutex<Option<Utf8PathBuf>>,
}

impl ReadFence {
    // Only catch-up, which the host build has, fences a store.
    #[cfg_attr(not(feature = "host"), allow(dead_code))]
    pub(super) const fn new(scope_dir: Utf8PathBuf, directories: Vec<&'static str>) -> Self {
        Self {
            scope_dir,
            directories,
            outside: Mutex::new(None),
        }
    }

    /// Refuses `path` when it is not below one of the directories, and
    /// keeps the first such path.
    pub(super) fn check(&self, path: &Utf8Path) -> anyhow::Result<()> {
        if self.allows(path) {
            return Ok(());
        }
        self.outside().get_or_insert_with(|| path.to_owned());
        Err(outside_error(path))
    }

    /// Whether `path` is below one of the directories. The parts of a
    /// shard path have `/` or the native separator between them.
    fn allows(&self, path: &Utf8Path) -> bool {
        let Ok(below) = path.strip_prefix(&self.scope_dir) else {
            return false;
        };
        let directory = below.as_str().split(['/', '\\']).next().unwrap_or_default();
        self.directories.contains(&directory)
    }

    fn outside(&self) -> std::sync::MutexGuard<'_, Option<Utf8PathBuf>> {
        // Each change replaces the whole value, so a poisoned value is intact.
        self.outside.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A store over one view that reads only the input directories of one
/// family reader. Only [`super::ViewStore::fenced`] makes one.
pub struct FencedStore(pub(super) GuardedStore<'static>);

impl FencedStore {
    /// Runs `read` on this store. A read outside the fence fails the result.
    pub(crate) fn read<R>(
        self,
        read: impl FnOnce(&GuardedStore<'_>) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let result = read(&self.0);
        let outside = self.0.read_fence().and_then(|fence| fence.outside().take());
        outside.map_or(result, |path| Err(outside_error(&path)))
    }
}

fn outside_error(path: &Utf8Path) -> anyhow::Error {
    crate::refusal::fault!("a family reader read {path}, which is outside its input directories")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Implementation aid: the fence refuses a read outside its directories,
    /// also when the reader drops the error. No Rule binds it.
    #[test]
    fn a_read_outside_the_fence_fails_the_body() {
        let scope = Utf8PathBuf::from("/repo/.provenance/state/scopes/default");
        let inside = scope.join("ideation/landings.jsonl");
        let outside = scope.join("rules/rule.jsonl");
        let fence = ReadFence::new(scope.clone(), vec!["ideation"]);
        assert!(fence.check(&inside).is_ok());
        assert!(fence.check(&scope.join("ideation")).is_ok());
        let error = fence.check(&outside).unwrap_err();
        assert_eq!(error.to_string(), outside_error(&outside).to_string());
        assert_eq!(fence.outside().take(), Some(outside));
        assert!(ReadFence::new(scope.clone(), vec![]).check(&scope).is_err());
    }
}
