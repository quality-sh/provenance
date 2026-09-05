//! The policy a query read runs under.
//!
//! No configuration file fills it yet; the reserved names for that file
//! are `read.freshness_policy` and `read.scan_limit`. There is no journal
//! knob, no visit budget, and no request-side knob.

use provenance_core::protocol::StampPolicy;

/// The default file count the working-tree scan stops at. One second
/// covers about 650 files on this repository, so 2000 bounds the scan near
/// three and a half seconds.
pub const DEFAULT_SCAN_LIMIT: usize = 2000;

/// Which freshness step a read runs before it answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FreshnessPolicy {
    /// Run catch-up under the publication guard, then answer.
    #[default]
    CatchUp,
    /// Answer at the stored serial without a freshness step.
    AnnotateOnly,
    /// Reserved and not implemented yet: refuse when the projection is
    /// behind.
    RefuseStale,
}

impl FreshnessPolicy {
    /// The stamp word for a step that ran as asked.
    pub const fn word(self) -> StampPolicy {
        match self {
            Self::CatchUp => StampPolicy::CatchUp,
            Self::AnnotateOnly => StampPolicy::AnnotateOnly,
            Self::RefuseStale => StampPolicy::RefuseStale,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadPolicy {
    pub freshness: FreshnessPolicy,
    /// The file count the working-tree scan behind `impact` stops at.
    pub scan_limit: usize,
}

impl ReadPolicy {
    pub const fn with_freshness(freshness: FreshnessPolicy) -> Self {
        Self {
            freshness,
            scan_limit: DEFAULT_SCAN_LIMIT,
        }
    }
}

impl Default for ReadPolicy {
    fn default() -> Self {
        Self::with_freshness(FreshnessPolicy::CatchUp)
    }
}
