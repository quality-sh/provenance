//! Deterministic pull request report rendering.
//!
//! Layers stay separate: an analysis layer (later work) builds the versioned
//! report envelope from graph snapshots, coverage scans and run records; this
//! module renders an envelope as bounded Markdown or normalized JSON. The
//! renderer consumes structured facts only. It calls no language model, reads
//! no conversational history, and performs no network write.

pub mod catalog;
pub mod envelope;
pub mod escape;
pub mod render;
