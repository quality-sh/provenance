//! The canonical record families: one row per record kind, and the views
//! that the engine, the projection and the catalog expand from the rows.
//!
//! The rows name record types, shard paths and readers. They do not need a
//! projection, so the engine builds them without the `host` feature.

pub(crate) mod family_table;
pub(crate) mod family_views;
// The projection readers and digests in this module serve the projection,
// which only the host build has.
#[cfg_attr(not(feature = "host"), allow(dead_code))]
pub(crate) mod projection_families;
mod review_facts;
pub(crate) mod review_families;

pub(crate) use family_views::record_families;
pub use projection_families::ProjectionFamily;
pub(crate) use projection_families::{BudgetKind, RecordFamily};
