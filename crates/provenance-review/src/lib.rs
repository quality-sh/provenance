//! This crate is an implementation part of Provenance. Provenance is pre-release, and this API can
//! change without compatibility shims. Use `provenance-sdk` for Rust.
//!
//! The review host, review links, the native follow stream, the notify client for a running
//! review host, and the review page bundle. The library returns typed values and takes a
//! stop future; the application that uses it owns the process, the signals, the terminal
//! and the browser.

#![deny(clippy::exit)]

pub mod assets;
pub mod host;
pub mod launch_key;
pub mod link;
pub mod notify;
pub mod owner_file;
pub mod repository;
pub mod reviewer;

pub use assets::{bundle, serve};
pub use host::{follow::follow, start, HostOptions, StartedHost, Startup};
pub use link::{
    issue_launch_code, review_location, AffectedReviewRecord, PageResult, ReviewLocation,
};
pub use notify::HostNotifyPort;
pub use repository::RepositoryScope;

/// Issues a review launch code to a caller that sends the launch key.
pub const LAUNCH_CODE_ROUTE: &str = "/review-launch/code";
/// Exchanges a review launch code for the page session one time.
pub const LAUNCH_SESSION_ROUTE: &str = "/review-launch/session";
