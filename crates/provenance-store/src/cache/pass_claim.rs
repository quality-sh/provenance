//! One catch-up pass at a time on the read database of one repository.
//!
//! Each pass that writes the read database holds the claim of its
//! repository while it runs: a catch-up of a read, `catch_up_state`,
//! `catch_up_in_publish` and the rebuilds of `materialize`. A second pass
//! that starts while the claim is held refuses at once with
//! [`CatchUpRunning`]. It does not wait, and it writes nothing.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The refusal of a catch-up pass that starts while another pass of the
/// same repository writes its read database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("another catch-up pass of this repository is running")]
pub struct CatchUpRunning;

/// Whether a catch-up pass of one repository runs. Each clone of the
/// repository shares it.
#[derive(Debug, Default)]
pub(crate) struct PassSlot(AtomicBool);

impl PassSlot {
    /// The claim of the one pass, or the refusal when another pass holds it.
    pub(crate) fn claim(self: &Arc<Self>) -> Result<PassClaim, CatchUpRunning> {
        if self.0.swap(true, Ordering::AcqRel) {
            return Err(CatchUpRunning);
        }
        Ok(PassClaim(Arc::clone(self)))
    }
}

/// The claim of the running pass. Dropping it frees the slot.
#[derive(Debug)]
pub(crate) struct PassClaim(Arc<PassSlot>);

impl Drop for PassClaim {
    fn drop(&mut self) {
        self.0 .0.store(false, Ordering::Release);
    }
}
