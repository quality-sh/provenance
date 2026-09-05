//! Readers over the projection tables, behind the snapshot handles.
//!
//! `rows` turns one `SQLite` row into a record through `ProjectionRow`,
//! `records` gives `Table<K>` its lookups, and `front` fetches one hop of
//! the `relations` table as a `RelationSource` the traversal core walks.
//! Nothing here touches a pool: every statement runs on the snapshot's
//! one transaction.

mod front;
mod records;
mod rows;

pub use front::SqlFront;
pub use records::kind_of;

/// The most bind parameters one statement takes: `SQLite` bounds them,
/// and a frontier or an id list is chunked to this size.
const BIND_CHUNK: usize = 500;
