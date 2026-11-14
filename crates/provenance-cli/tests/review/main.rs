//! The review tests of provenance-cli, compiled as one test binary.
//!
//! Each module is a test file in `tests/`. Each test binary links the full
//! dependency tree, so a small number of binaries keeps the coverage build
//! small. A support module that more than one file uses is declared here
//! once, and those files import it.

#[path = "../support"]
mod support {
    pub mod record_review;
    pub mod review_paths;
}
#[path = "../cli_agent_review_paths.rs"]
mod cli_agent_review_paths;
#[path = "../cli_review_accepted_citation.rs"]
mod cli_review_accepted_citation;
#[path = "../cli_review_detach.rs"]
mod cli_review_detach;
#[path = "../cli_review_detach_refusals.rs"]
mod cli_review_detach_refusals;
#[path = "../cli_review_discussion_no_op.rs"]
mod cli_review_discussion_no_op;
#[path = "../cli_review_document_choice.rs"]
mod cli_review_document_choice;
#[path = "../cli_review_follow_protocol.rs"]
mod cli_review_follow_protocol;
#[path = "../cli_review_follow_publication.rs"]
mod cli_review_follow_publication;
#[path = "../cli_review_follow_recovery.rs"]
mod cli_review_follow_recovery;
#[path = "../cli_review_git_starts.rs"]
mod cli_review_git_starts;
#[path = "../cli_review_host.rs"]
mod cli_review_host;
#[path = "../cli_review_launch.rs"]
mod cli_review_launch;
#[path = "../cli_review_links.rs"]
mod cli_review_links;
#[path = "../cli_review_navigation.rs"]
mod cli_review_navigation;
#[path = "../cli_review_notify.rs"]
mod cli_review_notify;
#[path = "../cli_review_notify_security.rs"]
mod cli_review_notify_security;
#[path = "../cli_review_open.rs"]
mod cli_review_open;
#[path = "../cli_review_refusal_connections.rs"]
mod cli_review_refusal_connections;
#[path = "../cli_review_rejected_records.rs"]
mod cli_review_rejected_records;
#[path = "../cli_review_rejection.rs"]
mod cli_review_rejection;
#[path = "../cli_review_resolution_owner.rs"]
mod cli_review_resolution_owner;
#[path = "../cli_review_reuse.rs"]
mod cli_review_reuse;
#[path = "../cli_review_reviewer.rs"]
mod cli_review_reviewer;
#[path = "../cli_review_session.rs"]
mod cli_review_session;
#[path = "../cli_review_startup.rs"]
mod cli_review_startup;
#[path = "../cli_review_submission_characterization.rs"]
mod cli_review_submission_characterization;
#[path = "../cli_review_uninitialized.rs"]
mod cli_review_uninitialized;
#[path = "../cli_review_unknown_routes.rs"]
mod cli_review_unknown_routes;
#[cfg(target_os = "linux")]
#[path = "../review_host_support/codex.rs"]
mod codex;
#[cfg(unix)]
#[path = "../review_host_support/persistent_browser.rs"]
mod persistent_browser;
#[path = "../porcelain_authoring_support/mod.rs"]
mod porcelain_authoring_support;
#[path = "../review_archive.rs"]
mod review_archive;
#[path = "../review_asset_routes.rs"]
mod review_asset_routes;
#[allow(dead_code)]
#[path = "../review_host_support/mod.rs"]
mod review_host_support;
#[path = "../review_native_events_probe.rs"]
mod review_native_events_probe;
#[path = "../review_shutdown.rs"]
mod review_shutdown;
#[path = "../review_windows_permissions.rs"]
mod review_windows_permissions;
