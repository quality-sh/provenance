//! Readers over the projection tables, behind the snapshot handles.
//!
//! `rows` turns one `SQLite` row into a record through `ProjectionRow`,
//! `records` gives `Table<K>` its lookups, and `front` fetches one hop of
//! the `relations` table as a `RelationSource` the traversal core walks.
//! Nothing here touches a pool: every statement runs on the snapshot's
//! one transaction.
//!
//! The readers build without `host`, but only the queries, which need
//! `host`, call most of them.
#![cfg_attr(not(feature = "host"), allow(dead_code))]

mod discussions;
mod document;
mod front;
pub(crate) mod page;
pub(crate) mod payloads;
mod records;
mod rows;

pub use front::SqlFront;
pub use records::kind_of;
#[cfg(test)]
#[cfg(feature = "host")]
pub(crate) use rows::{column_values, select_columns};
