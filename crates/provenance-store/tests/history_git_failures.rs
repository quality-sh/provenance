//! The failures and the commit names of a text read of the git adapter over
//! a real Git repository.
//!
//! Implementation aid with no Rule binding: no Rule states the faults of the
//! git adapter. These tests pin each refusal of a text read, and that a read
//! after a Git failure starts a new process. The contract states that a name
//! that is not a commit id of the adapter gives no text; these tests also pin
//! the names that the shared contract does not reach: the length of a commit
//! id of a SHA-256 repository, and upper case.

#[path = "history_git/repository.rs"]
mod repository;

use provenance_store::review::history::{FileVersions, GitFileVersions};
use repository::{git, repository, repository_in};
use std::path::Path;

#[tokio::test]
async fn a_name_with_a_line_break_is_refused() {
    let temp = tempfile::tempdir().unwrap();
    let (root, commit) = repository(&temp, &[("good.jsonl", "good\n")]);
    let versions = GitFileVersions::new(root);
    for path in ["go\nod.jsonl", "good.jsonl\r"] {
        let error = versions.text(path, &commit).await.unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            "git cat-file cannot read a name with a line break",
            "{path:?}"
        );
    }
    let text = versions.text("good.jsonl", &commit).await.unwrap();
    assert_eq!(text.as_deref(), Some("good\n"));
}

/// Git takes only a carriage return at the end of a request as part of its
/// line break, so one inside a name is a part of the name.
#[tokio::test]
async fn a_carriage_return_inside_a_name_is_part_of_the_name() {
    let temp = tempfile::tempdir().unwrap();
    let (root, commit) = repository(&temp, &[("good.jsonl", "good\n")]);
    let versions = GitFileVersions::new(root);
    let text = versions.text("go\rod.jsonl", &commit).await.unwrap();
    assert_eq!(text, None);
}

#[tokio::test]
async fn a_git_failure_before_an_answer_gives_the_git_diagnostic() {
    let temp = tempfile::tempdir().unwrap();
    let (root, commit) = repository(&temp, &[("bad.jsonl", "bad\n")]);
    unknown_type(temp.path(), &commit, "bad.jsonl");
    let versions = GitFileVersions::new(root);
    let error = format!(
        "{:#}",
        versions.text("bad.jsonl", &commit).await.unwrap_err()
    );
    assert!(
        error.starts_with("git cat-file failed: ") && error.contains("fatal:"),
        "{error}"
    );
}

/// The process stops at a failure, so the next text starts a new one.
#[tokio::test]
async fn a_text_after_a_git_failure_starts_a_new_process() {
    let temp = tempfile::tempdir().unwrap();
    let files = [("good.jsonl", "good\n"), ("bad.jsonl", "bad\n")];
    let (root, commit) = repository(&temp, &files);
    unknown_type(temp.path(), &commit, "bad.jsonl");
    let versions = GitFileVersions::new(root);
    let good = versions.text("good.jsonl", &commit).await.unwrap();
    assert_eq!(good.as_deref(), Some("good\n"));
    assert!(versions.text("bad.jsonl", &commit).await.is_err());
    let good = versions.text("good.jsonl", &commit).await.unwrap();
    assert_eq!(good.as_deref(), Some("good\n"));
}

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

/// Makes the stored object of `path` at `commit` an object of a type that
/// Git does not know, so that `git cat-file` stops before it answers.
pub fn unknown_type(root: &Path, commit: &str, path: &str) {
    let object = git(root, &["rev-parse", &format!("{commit}:{path}")]);
    let file = root
        .join(".git/objects")
        .join(&object[..2])
        .join(&object[2..]);
    // Git makes an object file read-only; Windows does not remove such a file.
    #[cfg(windows)]
    {
        let mut permissions = std::fs::metadata(&file).unwrap().permissions();
        permissions.set_readonly(false);
        std::fs::set_permissions(&file, permissions).unwrap();
    }
    std::fs::remove_file(&file).unwrap();
    std::fs::write(&file, stored_zlib(b"blub 5\0hello")).unwrap();
}

/// `data` as a zlib stream of one stored block.
fn stored_zlib(data: &[u8]) -> Vec<u8> {
    let length = u16::try_from(data.len()).unwrap();
    let mut stream = vec![0x78, 0x01, 0x01];
    stream.extend(length.to_le_bytes());
    stream.extend((!length).to_le_bytes());
    stream.extend(data);
    let (mut low, mut high) = (1_u32, 0_u32);
    for byte in data {
        low = (low + u32::from(*byte)) % 65521;
        high = (high + low) % 65521;
    }
    stream.extend(((high << 16) | low).to_be_bytes());
    stream
}
