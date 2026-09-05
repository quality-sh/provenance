//! Frozen copies of the operations as they read canonical shards before
//! any operation flipped to the projection. The comparison tests run
//! each served operation against its copy over the same store.

pub mod bindings;
pub mod evidence;
pub mod impact;
pub mod records;
pub mod stale;
pub mod symbols;
pub mod walk;
