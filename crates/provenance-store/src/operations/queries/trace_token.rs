//! Trace resume tokens.
//!
//! A trace that truncates mid-breadth returns a resume token carrying the
//! depth plus rank-plus-id watermark it stopped behind. Resuming replays
//! the deterministic walk, skips everything at or before the watermark,
//! and continues, so the final resumed walk equals an untruncated run at
//! the same max depth with no duplicate `TracedNode` across the boundary.
//! A token names the request it was cut from: a resume whose parameters
//! differ is refused.

use anyhow::Context as _;
use provenance_core::protocol::Direction;
use provenance_core::{EdgeType, StableId};

/// The watermark one truncated trace page stopped behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeToken {
    pub depth: usize,
    pub rank: u8,
    pub id: String,
    /// Hash binding the token to the request that produced it.
    pub fingerprint: String,
}

/// Binds a token to the walk semantics of the request that produced it.
///
/// The page size is deliberately absent: resuming with a larger page is
/// exactly the point of a resume token.
pub fn fingerprint(
    id: &str,
    direction: Direction,
    edge_types: &[EdgeType],
    max_depth: usize,
) -> String {
    let mut names: Vec<&str> = edge_types.iter().map(|edge| edge_word(*edge)).collect();
    names.sort_unstable();
    // ';' separates fingerprint fields so the token's '|' delimiter stays
    // unambiguous.
    format!(
        "{id};{:?};{};{max_depth}",
        direction_discriminant(direction),
        names.join(";")
    )
}

const fn direction_discriminant(direction: Direction) -> u8 {
    match direction {
        Direction::Out => 0,
        Direction::In => 1,
        Direction::Both => 2,
    }
}

const fn edge_word(edge_type: EdgeType) -> &'static str {
    match edge_type {
        EdgeType::References => "references",
        EdgeType::RefinesInto => "refines_into",
        EdgeType::DependsOn => "depends_on",
        EdgeType::Contradicts => "contradicts",
        EdgeType::Supersedes => "supersedes",
        EdgeType::Needs => "needs",
        EdgeType::Resolves => "resolves",
        EdgeType::Spawns => "spawns",
        EdgeType::Produces => "produces",
    }
}

impl ResumeToken {
    /// Encodes the token for the wire.
    pub fn encode(&self) -> String {
        // Fields are length-safe: the id is a canonical id word and the
        // fingerprint is hex, so `|` is an unambiguous separator.
        format!(
            "trv1|{}|{}|{}|{}",
            self.depth, self.rank, self.id, self.fingerprint
        )
    }

    /// Decodes a token from the wire.
    pub fn decode(token: &str) -> anyhow::Result<Self> {
        let parts: Vec<&str> = token.split('|').collect();
        if parts.len() != 5 || parts[0] != "trv1" {
            anyhow::bail!("unknown trace token format");
        }
        let depth: usize = parts[1].parse().context("trace token depth")?;
        let rank: u8 = parts[2].parse().context("trace token rank")?;
        Ok(Self {
            depth,
            rank,
            id: parts[3].to_string(),
            fingerprint: parts[4].to_string(),
        })
    }

    /// Whether this token may continue a request with these parameters.
    pub fn matches(&self, fingerprint: &str) -> bool {
        self.fingerprint == fingerprint
    }

    /// Whether a reached node comes strictly after this watermark in the
    /// deterministic walk order: greater depth, or the same depth with a
    /// greater rank-plus-id key.
    pub fn precedes(&self, depth: usize, node_rank: u8, id: &StableId) -> bool {
        (self.depth, self.rank, self.id.as_str()) < (depth, node_rank, id.as_str())
    }
}
