//! The commit names that the git adapter reads a text at.
//!
//! Implementation aid with no Rule binding: the contract states that a name
//! that is not a commit id of the adapter gives no text. These tests pin the
//! names of the git adapter that the shared contract does not reach: the
//! length of a commit id of a SHA-256 repository, and upper case.

#[path = "history_git/repository.rs"]
mod repository;

use provenance_store::review::history::{FileVersions, GitFileVersions};
use repository::{repository, repository_in};

/// Git reads upper-case digits as the same commit id.
#[tokio::test]
async fn an_upper_case_commit_id_names_the_commit() {
    let temp = tempfile::tempdir().unwrap();
    let (root, commit) = repository(&temp, &[("good.jsonl", "good\n")]);
    let versions = GitFileVersions::new(root);
    let text = versions
        .text("good.jsonl", &commit.to_uppercase())
        .await
        .unwrap();
    assert_eq!(text.as_deref(), Some("good\n"));
}

/// Git reads the first 40 digits of a SHA-256 commit id as an abbreviated
/// id, so the adapter gives no text for them.
#[tokio::test]
async fn a_sha1_length_name_in_a_sha256_repository_has_no_text() {
    let temp = tempfile::tempdir().unwrap();
    let init = ["init", "-q", "--object-format=sha256"];
    let (root, commit) = repository_in(&temp, &init, &[("good.jsonl", "good\n")]);
    assert_eq!(commit.len(), 64);
    let versions = GitFileVersions::new(root);
    let listed = versions.versions("good.jsonl").await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].commit, commit);
    let text = versions.text("good.jsonl", &commit).await.unwrap();
    assert_eq!(text.as_deref(), Some("good\n"));
    let text = versions.text("good.jsonl", &commit[..40]).await.unwrap();
    assert_eq!(text, None);
}
