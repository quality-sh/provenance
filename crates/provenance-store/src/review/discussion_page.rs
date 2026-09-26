//! One discussion page as it fills, inside its entry limit and the page
//! byte budget.

use crate::operations::reader::{PAGE_BYTES, RECORD_BYTES};
use provenance_core::protocol::read_failure::ReadFailure;

/// The page bytes kept free for the envelope and the cursor.
const ENVELOPE_BYTES: usize = 16_384;

/// Whether a read added every row, or stopped because the page is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Fill {
    Complete,
    Full,
}

pub(super) struct DiscussionPageFill<T> {
    pub(super) entries: Vec<T>,
    bytes: usize,
    limit: usize,
}

impl<T: serde::Serialize> DiscussionPageFill<T> {
    pub(super) const fn new(limit: usize) -> Self {
        Self {
            entries: Vec::new(),
            bytes: 0,
            limit,
        }
    }

    /// True when the page holds its limit of entries.
    pub(super) const fn at_limit(&self) -> bool {
        self.entries.len() == self.limit
    }

    /// True when `size` more bytes would pass the page byte budget.
    pub(super) const fn over_budget(&self, size: usize) -> bool {
        self.bytes + size > PAGE_BYTES - ENVELOPE_BYTES
    }

    /// The serialized size of one entry. An entry over the record budget
    /// refuses the page.
    pub(super) fn record_size(entry: &T) -> anyhow::Result<usize> {
        let size = serde_json::to_vec(entry)?.len();
        if size > RECORD_BYTES {
            return Err(ReadFailure::PageRecordTooLarge.into());
        }
        Ok(size)
    }

    pub(super) fn push(&mut self, entry: T, size: usize) {
        self.bytes += size + 1;
        self.entries.push(entry);
    }
}

/// Refuses a stored row whose byte length is over the record budget, before
/// the read loads it.
pub(super) fn check_stored_size(size: i64) -> anyhow::Result<()> {
    if size > i64::try_from(RECORD_BYTES)? {
        return Err(ReadFailure::PageRecordTooLarge.into());
    }
    Ok(())
}
