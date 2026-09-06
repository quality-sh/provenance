//! The resolved settings for one query read.

use provenance_core::protocol::StampPolicy;
use provenance_macros::rule;

/// The scan stops at 5000 source files, about 50 MB at 10 KB per file.
///
/// Release scans measured 8.6 to 11.4 ms per MB, about half a second at
/// that size. Larger files can take longer. W5 plan revision 3, section D:
/// ripgrep 3fce3b5: 110 files, 1.9 MB, release 17.5 ms, debug 121.0 ms;
/// provenance 03a935c: 740 files, 4.3 MB, release 49.1 ms, debug 333.8 ms;
/// tokio 060cc4e: 796 files, 5.7 MB, release 54.2 ms, debug 378.1 ms;
/// rust-analyzer 04a5986: 1511 files, 17.7 MB, release 152.8 ms, debug 1275.5 ms.
///
/// To repeat: initialize an empty scope in each checkout; time `sdk impact`
/// for an absent id and `sdk get` six times in each build. Subtract the
/// median get time from the median impact time after one warm-up run.
pub const DEFAULT_SCAN_LIMIT: usize = 5000;

/// Which freshness step a read runs before it answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
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
    /// The flag selects freshness before the file and the built-in default.
    #[rule("rule_freshness_flag_wins_over_the_settings_file")]
    pub fn resolve(
        settings: &crate::settings::Settings,
        freshness: Option<FreshnessPolicy>,
    ) -> Self {
        Self {
            freshness: freshness
                .or(settings.read.freshness_policy)
                .unwrap_or_default(),
            scan_limit: settings.read.scan_limit.unwrap_or(DEFAULT_SCAN_LIMIT),
        }
    }

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
