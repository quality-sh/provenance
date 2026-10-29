//! The holders of publication sections and staged sections.
//!
//! An entry records that a thread holds a section. The table only refuses a
//! second entry on the same thread: a nested entry would wait on its own lock.
//! It grants nothing. A section that holds the lock hands its closure a held
//! store, and only that store reads and writes without a second lock.

use camino::Utf8PathBuf;
use provenance_macros::rule;
use std::sync::{Mutex, PoisonError};
use std::thread::ThreadId;

/// A section of this repository is already held on this thread.
#[derive(Debug, thiserror::Error)]
#[error("a publication section of {repository} is already held on this thread")]
pub struct PublicationHeld {
    pub repository: Utf8PathBuf,
}

/// The kind of section an entry holds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HolderKind {
    /// A live publication section, under the repository publication lock.
    Live,
    /// A staged section, under the lock that its stage owns.
    Stage,
}

type Key = (String, HolderKind, ThreadId);

// A thread holds few sections at once, so a list is enough.
static HOLDERS: Mutex<Vec<Key>> = Mutex::new(Vec::new());

/// Refuses when this thread already holds the section with `identity`.
#[rule("rule_publication_holders_grant_nothing")]
pub(crate) fn refuse_if_held(
    identity: &str,
    kind: HolderKind,
    repository: &camino::Utf8Path,
) -> anyhow::Result<()> {
    let key = (identity.to_owned(), kind, std::thread::current().id());
    if table().contains(&key) {
        return Err(PublicationHeld {
            repository: repository.to_owned(),
        }
        .into());
    }
    Ok(())
}

/// Records that this thread holds the section with `identity` until the
/// entry drops, also on unwind.
pub(crate) fn enter(identity: String, kind: HolderKind) -> HolderEntry {
    let key = (identity, kind, std::thread::current().id());
    table().push(key.clone());
    HolderEntry { key }
}

pub(crate) struct HolderEntry {
    key: Key,
}

impl Drop for HolderEntry {
    fn drop(&mut self) {
        let mut table = table();
        if let Some(at) = table.iter().position(|key| key == &self.key) {
            table.swap_remove(at);
        }
    }
}

fn table() -> std::sync::MutexGuard<'static, Vec<Key>> {
    // An entry is inserted or removed in one step, so a poisoned table is intact.
    HOLDERS.lock().unwrap_or_else(PoisonError::into_inner)
}
