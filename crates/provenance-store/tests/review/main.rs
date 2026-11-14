//! The review tests of provenance-store, compiled as one test binary.
//!
//! Each module is a test file in `tests/`. Each test binary links the full
//! dependency tree, so a small number of binaries keeps the coverage build
//! small. A support module that more than one file uses is declared here
//! once, and those files import it.

#[path = "../automatic_submissions.rs"]
mod automatic_submissions;
#[path = "../discussion_support.rs"]
mod discussion_support;
#[path = "../native_review_submissions.rs"]
mod native_review_submissions;
#[path = "../requirement_review_reads.rs"]
mod requirement_review_reads;
#[path = "../review_cycle_records.rs"]
mod review_cycle_records;
#[path = "../review_history.rs"]
mod review_history;
#[path = "../review_history_characterization.rs"]
mod review_history_characterization;
#[path = "../review_history_git.rs"]
mod review_history_git;
#[path = "../review_markers.rs"]
mod review_markers;
#[path = "../review_paths.rs"]
mod review_paths;
#[path = "../review_projection.rs"]
mod review_projection;
#[path = "../review_reads.rs"]
mod review_reads;
#[path = "../review_revision_kernel.rs"]
mod review_revision_kernel;
#[path = "../review_saves.rs"]
mod review_saves;
#[path = "../review_validation.rs"]
mod review_validation;
