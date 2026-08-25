//! Production code with no Provenance awareness.

use std::time::{Duration, SystemTime};

const SEVEN_DAYS: Duration = Duration::from_secs(7 * 24 * 60 * 60);

pub struct ShareLink {
    pub expires_at: SystemTime,
}

pub fn create_share_link(now: SystemTime) -> ShareLink {
    ShareLink {
        expires_at: now + SEVEN_DAYS,
    }
}
