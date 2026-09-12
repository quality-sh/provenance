//! Deterministic pull request report rendering and envelope production.
//!
//! Layers stay separate: the builder module writes the versioned report
//! envelope from a real repository scan and the committed graph; this
//! module renders an envelope as bounded Markdown or normalized JSON. The
//! renderer consumes structured facts only. It calls no language model,
//! reads no conversational history, and performs no network write.

pub mod build;
pub mod catalog;
pub mod envelope;
pub mod escape;
pub mod render;
