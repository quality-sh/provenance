//! One catch-up pass at a time on one read database.
//!
//! Each pass that writes the read database holds the claim of that
//! database while it runs: a catch-up of a read, `catch_up_state`,
//! `catch_up_in_publish` and the rebuilds of `materialize`. A second pass
//! that starts while the claim is held refuses at once with
//! [`CatchUpRunning`]. It does not wait, and it writes nothing.
//!
//! The claim is kept beside the read database, not in one repository
//! handle: each handle over the same read database shares it. The port of
//! the read database does not say what two writes at once do, so the
//! claim keeps them apart.

use crate::read_database::ReadDatabase;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

/// The refusal of a catch-up pass that starts while another pass writes
/// the same read database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("another catch-up pass of this repository is running")]
pub struct CatchUpRunning;

/// Whether a catch-up pass writes one read database. Each repository handle
/// over that database shares it.
#[derive(Debug, Default)]
pub struct PassSlot(AtomicBool);

/// The slot of each read database that a repository handle holds. An entry
/// goes when its read database is dropped.
static SLOTS: Mutex<Vec<(Weak<dyn ReadDatabase>, Arc<PassSlot>)>> = Mutex::new(Vec::new());

impl PassSlot {
    /// The slot of the passes that write `database`. Each handle over the
    /// same read database gets the same slot. A handle with no read
    /// database gets a slot of its own: it has no pass to keep apart.
    pub fn of(database: Option<&Arc<dyn ReadDatabase>>) -> Arc<Self> {
        let Some(database) = database else {
            return Arc::default();
        };
        let mut slots = SLOTS.lock().unwrap_or_else(PoisonError::into_inner);
        slots.retain(|(kept, _)| kept.strong_count() > 0);
        let shared = slots
            .iter()
            .find(|(kept, _)| std::ptr::addr_eq(kept.as_ptr(), Arc::as_ptr(database)))
            .map(|(_, slot)| Arc::clone(slot));
        shared.unwrap_or_else(|| {
            let slot = Arc::<Self>::default();
            slots.push((Arc::downgrade(database), Arc::clone(&slot)));
            slot
        })
    }

    /// The claim of the one pass, or the refusal when another pass holds it.
    pub fn claim(self: &Arc<Self>) -> Result<PassClaim, CatchUpRunning> {
        if self.0.swap(true, Ordering::AcqRel) {
            return Err(CatchUpRunning);
        }
        Ok(PassClaim(Arc::clone(self)))
    }
}

/// The claim of the running pass. Dropping it frees the slot.
#[derive(Debug)]
pub struct PassClaim(Arc<PassSlot>);

impl Drop for PassClaim {
    fn drop(&mut self) {
        self.0 .0.store(false, Ordering::Release);
    }
}
