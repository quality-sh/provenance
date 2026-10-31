//! The holders of publication sections, staged sections and stage bodies.
//!
//! An entry records that a thread holds a section or runs a stage body. The
//! table only refuses: a second section entry on the same thread would wait on
//! its own lock, and live access in a stage body would read around the stage.
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
pub enum HolderKind {
    /// A live publication section, under the repository publication lock.
    Live,
    /// A staged section, under the lock that its stage owns.
    Stage,
    /// The body of a stage of the live repository.
    StageBody,
}

type Key = (String, HolderKind, ThreadId);

// A thread holds few sections at once, so a list is enough.
static HOLDERS: Mutex<Vec<Key>> = Mutex::new(Vec::new());

/// Refuses when this thread already holds the section with `identity`.
#[rule("rule_publication_holders_grant_nothing")]
pub fn refuse_if_held(
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

/// Whether this thread has an entry of `kind` with `identity`, or of `kind`
/// with any identity when `identity` is `None`.
pub fn held_here(identity: Option<&str>, kind: HolderKind) -> bool {
    let thread = std::thread::current().id();
    table().iter().any(|(held, held_kind, held_thread)| {
        *held_kind == kind
            && *held_thread == thread
            && identity.is_none_or(|identity| held == identity)
    })
}

/// Records that this thread holds the section with `identity` until the
/// entry drops, also on unwind.
pub fn enter(identity: String, kind: HolderKind) -> HolderEntry {
    let key = (identity, kind, std::thread::current().id());
    table().push(key.clone());
    HolderEntry { key }
}

pub struct HolderEntry {
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
