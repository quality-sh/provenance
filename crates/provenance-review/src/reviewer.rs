//! The reviewer identity of the person who runs a review host, from the Git configuration.

use camino::Utf8Path;
use provenance_transport::reviewer::{git_actor_id, git_config};

/// The reviewer of the person who runs the host: `provenance.reviewer` from the Git
/// configuration, or else the Git identity.
#[provenance_macros::rule("rule_review_host_reports_reviewer")]
pub fn review_actor_id(path: &Utf8Path) -> Option<String> {
    git_config(path, "provenance.reviewer").or_else(|| git_actor_id(path))
}
