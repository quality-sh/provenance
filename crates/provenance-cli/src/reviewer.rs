//! Selects the initial human reviewer and describes the resulting setup.

use camino::Utf8Path;
use provenance_core::StableId;
use std::io::{IsTerminal, Write};

pub(super) struct InitialReviewer {
    pub(super) actor_id: Option<String>,
    pub(super) notice: String,
}

pub(super) fn select(path: &Utf8Path) -> anyhow::Result<InitialReviewer> {
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

pub(super) fn review_page_warning(path: &std::path::Path) -> String {
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

fn git_actor_id(path: &Utf8Path) -> Option<String> {
    let working_directory = path
        .ancestors()
        .find(|candidate| candidate.exists())
        .unwrap_or(path);
    ["user.email", "user.name"].into_iter().find_map(|key| {
        let output = std::process::Command::new("git")
            .args(["-C", working_directory.as_str(), "config", "--get", key])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let value = String::from_utf8(output.stdout).ok()?;
        normalize_actor_id(value.trim())
    })
}

fn normalize_actor_id(identity: &str) -> Option<String> {
    let mut normalized = String::new();
    let mut separator = false;
    for character in identity.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !normalized.is_empty() {
                normalized.push('_');
            }
            normalized.push(character.to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
    }
    StableId::new(normalized.clone()).ok().map(|_| normalized)
}
