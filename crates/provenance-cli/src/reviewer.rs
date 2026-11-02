//! Selects the initial human reviewer and describes the resulting setup.

use camino::Utf8Path;
use provenance_review::reviewer::git_actor_id;
use std::io::{IsTerminal, Write};

pub struct InitialReviewer {
    pub actor_id: Option<String>,
    pub notice: String,
}

/// Selects the human reviewer during repository initialization.
#[provenance_macros::rule("rule_init_configures_human_reviewer")]
pub fn select(path: &Utf8Path) -> anyhow::Result<InitialReviewer> {
    let git_identity = git_actor_id(path);
    let actor_id = if std::io::stdin().is_terminal() {
        Some(prompt(git_identity.as_deref().unwrap_or("reviewer"))?)
    } else {
        git_identity
    };
    let notice = actor_id.as_ref().map_or_else(
        || read_only_warning(path.as_std_path()),
        |actor_id| reviewer_set_notice(path, actor_id),
    );
    Ok(InitialReviewer { actor_id, notice })
}

fn read_only_warning(path: &std::path::Path) -> String {
    format!(
        "Warning: No reviewer is configured. Review will be read-only. Add a reviewer with `provenance init --path {} --disposition-actor-id <reviewer-id>`.",
        path.display()
    )
}

/// States that a review page without a configured reviewer is read-only.
#[provenance_macros::rule("rule_review_page_declares_read_only")]
pub fn review_page_warning(path: &std::path::Path) -> String {
    format!(
        "Warning: No reviewer is configured. The review page will be read-only. Add a reviewer with `provenance init --path {} --disposition-actor-id <reviewer-id>`.",
        path.display()
    )
}

fn reviewer_set_notice(path: &Utf8Path, actor_id: &str) -> String {
    format!(
        "Reviewer set to \"{actor_id}\". Change reviewers with `provenance init --path {path} --disposition-actor-id <reviewer-id>`."
    )
}

fn prompt(default: &str) -> anyhow::Result<String> {
    eprint!("Reviewer ID [{default}]: ");
    std::io::stderr().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(match input.trim() {
        "" => default.to_owned(),
        value => value.to_owned(),
    })
}
