//! Frozen copies of the operations as they read canonical shards before
//! they moved onto the projection. The comparison tests run each served
//! operation that still has a copy against it over the same store. The
//! walk, impact, evidence, and symbols copies went with their flips;
//! `records` stays until `records::load` goes, and `stale` stays with
//! the operation it copies, which reads no projection table.

pub mod records;
pub mod stale;
