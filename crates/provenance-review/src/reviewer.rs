//! The reviewer identity of the person who runs a review host, from the Git configuration.

use camino::Utf8Path;
use provenance_core::StableId;

/// The reviewer of the person who runs the host: `provenance.reviewer` from the Git
/// configuration, or else the Git identity.
#[provenance_macros::rule("rule_review_host_reports_reviewer")]
pub fn review_actor_id(path: &Utf8Path) -> Option<String> {
    git_config(path, "provenance.reviewer").or_else(|| git_actor_id(path))
}

/// The Git identity of the person as an actor ID: the e-mail address, or else the name,
/// in the stable identifier form.
pub fn git_actor_id(path: &Utf8Path) -> Option<String> {
    ["user.email", "user.name"]
        .into_iter()
        .find_map(|key| normalize_actor_id(&git_config(path, key)?))
}

fn git_config(path: &Utf8Path, key: &str) -> Option<String> {
    let working_directory = path
        .ancestors()
        .find(|candidate| candidate.exists())
        .unwrap_or(path);
    let output = std::process::Command::new("git")
        .current_dir(working_directory)
        .args(["config", "--get", key])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    Some(value.trim().to_owned()).filter(|value| !value.is_empty())
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
