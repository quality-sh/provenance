//! Freshness stamps: what an answer attests, and what stays live.
//!
//! Every served response carries a stamp. The stamp names the projection
//! instance and serial it was read behind, the digest over every stored
//! family, the freshness policy that produced it, the projection domains
//! the answer's fields come from, and the live constituents the stamp
//! does not cover. A stamp never implies freshness for a domain it does
//! not list.
//!
//! Serials carry no meaning across projection instances: after total
//! cache loss a fresh database restarts at serial 1 with a fresh instance
//! identifier, so clients must refuse serial comparison across
//! instances. The instance field is what makes that refusal possible.

use serde::{Deserialize, Serialize};

/// How the serving pass produced the stamp.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessPolicy {
    /// Materialize, then serve locally.
    #[default]
    CatchUp,
    /// Stamp without catching up; offline use.
    AnnotateOnly,
    /// Refuse with a typed staleness error when the stamp cannot be made
    /// current.
    RefuseStale,
}

/// The projection domains an answer's fields come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttestedDomain {
    /// Canonical graph records: sources, requirements, resolutions, rules,
    /// topics, questions, domains, boundaries, edges.
    Graph,
    /// Implementation and verification bindings.
    Bindings,
    /// Requirement reviews.
    Reviews,
}

/// A constituent an answer uses that the stamp does not attest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveConstituent {
    /// Verification runs live in cache JSONL outside the projection.
    VerificationRuns,
    /// The stale half reads a git diff the caller names.
    StaleDiff,
    /// Scanner sites come from the working tree, not the projection.
    ScannerSites,
    /// The whole answer derives from live state; nothing is attested.
    Unattested,
}

/// The freshness claim behind one served response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FreshnessStamp {
    /// The projection instance the serial belongs to. Clients must refuse
    /// serial comparison across instances.
    pub instance: String,
    /// The revision serial the read answered behind.
    pub serial: i64,
    /// The digest over every stored family at that serial.
    pub digest: String,
    /// The policy that produced this stamp.
    pub policy: FreshnessPolicy,
    /// The domains behind the answer's fields.
    pub attested: Vec<AttestedDomain>,
    /// The constituents the stamp does not cover.
    pub live: Vec<LiveConstituent>,
}

/// Encodes a paging offset as an opaque continuation token.
pub fn encode_cursor(offset: usize) -> String {
    format!("v1:{offset}")
}

/// Decodes a continuation token into its paging offset.
pub fn decode_cursor(token: &str) -> anyhow::Result<usize> {
    let rest = token
        .strip_prefix("v1:")
        .ok_or_else(|| anyhow::anyhow!("unknown cursor format"))?;
    let offset: usize = rest.parse()?;
    Ok(offset)
}

/// How many walk steps a traversal may expand by default.
pub const VISIT_BUDGET_DEFAULT: usize = 2_000;

/// The largest visit budget any request or config may set.
pub const VISIT_BUDGET_CAP: usize = 10_000;

/// How many scanned sites a live-scan half may consume by default.
pub const SCAN_BUDGET_DEFAULT: usize = 2_000;

/// The largest scan budget any request or config may set.
pub const SCAN_BUDGET_CAP: usize = 10_000;

/// Resolves an optional override against a default and its cap: requests
/// may override downward within caps.
pub const fn resolve_budget(requested: Option<usize>, default: usize, cap: usize) -> usize {
    let value = match requested {
        Some(value) if value < default => value,
        _ => default,
    };
    if value > cap {
        cap
    } else {
        value
    }
}
