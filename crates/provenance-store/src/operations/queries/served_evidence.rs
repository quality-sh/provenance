//! Served `evidence`: everything standing behind one Rule.
//!
//! Bindings and reviews come from the projection; verification runs stay
//! cache JSONL; the stale half reads the git diff the caller names. Every
//! collection pages truthfully: per-collection `has_more` flags and
//! per-collection continuation tokens, with the top-level fields kept.

pub(super) use super::served_live::evidence;
