//! Record history when `HEAD` moves while a history read runs. A `git` first on
//! PATH moves `HEAD` right after the read resolves it, so the read must use the
//! commit that it resolved for every later Git command and for its cache.
#![cfg(unix)]
#[allow(dead_code)]
mod review_support;
use provenance_core::review::ReviewHistoryQuery;
use provenance_core::NodeType;
use provenance_macros::verifies;
use provenance_store::{operations::read_policy::ReadPolicy, review::read_history};
use review_support::*;
use serde_json::json;
use std::{os::unix::fs::PermissionsExt as _, path::Path};

/// Puts a `git` first on PATH that runs the real Git. After a run that resolves
/// the history prefix, it moves `HEAD` of `repo` to the commit in `arm`, once.
fn moving_git(directory: &Path, repo: &Path, arm: &Path) {
    let real = std::process::Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    let real = String::from_utf8(real.stdout).unwrap();
    let real = real.trim();
    let wrapper = directory.join("git");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\n'{real}' \"$@\"\nstatus=$?\n\
             case \" $* \" in *' --show-prefix '*)\n\
             if [ -f '{arm}' ]; then target=$(cat '{arm}'); rm -f '{arm}'; \
             '{real}' -C '{repo}' update-ref HEAD \"$target\"; fi;;\nesac\nexit $status\n",
            arm = arm.display(),
            repo = repo.display(),
        ),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(
        std::iter::once(directory.to_path_buf()).chain(std::env::split_paths(&path)),
    )
    .unwrap();
    // This test binary holds one test, so no other thread reads PATH.
    std::env::set_var("PATH", path);
}

async fn version_ids(temp: &tempfile::TempDir) -> Vec<String> {
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let query = ReviewHistoryQuery {
        record_kind: NodeType::Requirement,
        record_id: id(),
        limit: 50,
        cursor: None,
    };
    read_history(&repository(root), ReadPolicy::default(), query)
        .await
        .unwrap()
        .result
        .entries
        .into_iter()
        .map(|version| version.id.as_str().to_owned())
        .collect()
}

#[tokio::test]
#[verifies("rule_record_history_reads_git", examples)]
async fn history_lists_the_commits_of_the_head_that_the_read_resolved() {
    let (temp, store) = fixture();
    let created = commit_state(&temp, "Create");
    store
        .save_requirement(save(
            &store,
            json!({"statement":"The system stores records for one year."}),
        ))
        .unwrap();
    let changed = commit_state(&temp, "Change");
    git(&temp, &["reset", "-q", "--hard", &created]);
    let tools = tempfile::tempdir().unwrap();
    let arm = tools.path().join("move-head-to");
    moving_git(tools.path(), temp.path(), &arm);

    // The read resolves `created`, then `HEAD` moves to `changed`.
    std::fs::write(&arm, &changed).unwrap();
    assert_eq!(version_ids(&temp).await, [created.as_str()]);
    assert_eq!(git(&temp, &["rev-parse", "HEAD"]), changed);

    // Back at `created`, the kept versions are the versions of `created`.
    git(&temp, &["update-ref", "HEAD", &created]);
    assert_eq!(version_ids(&temp).await, [created.as_str()]);

    // At `changed`, the read lists both commits. The saved copy is the record
    // of `created`, so it follows as the working version.
    git(&temp, &["update-ref", "HEAD", &changed]);
    assert_eq!(
        version_ids(&temp).await,
        [created.as_str(), changed.as_str(), "working"]
    );
}
