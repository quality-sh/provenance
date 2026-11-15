//! The discussion tests of provenance-store, compiled as one test binary.
//!
//! Each module is a test file in `tests/`. Each test binary links the full
//! dependency tree, so a small number of binaries keeps the coverage build
//! small. A support module that more than one file uses is declared here
//! once, and those files import it.

#[path = "../cursor_key_repair.rs"]
mod cursor_key_repair;
#[path = "../discussion_compatibility.rs"]
mod discussion_compatibility;
#[path = "../discussion_discovery.rs"]
mod discussion_discovery;
#[path = "../discussion_excerpt.rs"]
mod discussion_excerpt;
#[path = "../discussion_kinds.rs"]
mod discussion_kinds;
#[path = "../discussion_message_cursor.rs"]
mod discussion_message_cursor;
#[path = "../discussion_origins.rs"]
mod discussion_origins;
#[path = "../discussion_outcomes.rs"]
mod discussion_outcomes;
#[path = "../discussion_reads.rs"]
mod discussion_reads;
#[path = "../discussion_support.rs"]
mod discussion_support;
use discussion_support::base as review_support;
#[path = "../discussion_target_writes.rs"]
mod discussion_target_writes;
#[path = "../discussion_writes.rs"]
mod discussion_writes;
