//! The source tests of provenance-store, compiled as one test binary.
//!
//! Each module is a test file in `tests/`. Each test binary links the full
//! dependency tree, so a small number of binaries keeps the coverage build
//! small. A support module that more than one file uses is declared here
//! once, and those files import it.

#[path = "../support/memory.rs"]
mod memory;
#[path = "../refused_post.rs"]
mod refused_post;
#[path = "../store_shape.rs"]
mod store_shape;
#[path = "../worker_clock.rs"]
mod worker_clock;
