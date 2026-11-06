use super::{current_git_commit, scan_commit};
use camino::Utf8PathBuf;

#[test]
fn coverage_commit_is_read_from_the_selected_repository() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Utf8PathBuf::from_path_buf(dir.path().join("repo")).unwrap();
    run_git(["init", repo.as_str()]);
    std::fs::write(repo.join("tracked.txt"), "tracked\n").unwrap();
    run_git(["-C", repo.as_str(), "add", "tracked.txt"]);
    run_git([
        "-C",
        repo.as_str(),
        "-c",
        "user.name=Provenance Test",
        "-c",
        "user.email=test@example.invalid",
        "commit",
        "-m",
        "fixture",
    ]);

    let expected = git_stdout(["-C", repo.as_str(), "rev-parse", "HEAD"]);

    assert_eq!(current_git_commit(&repo).unwrap(), expected);
    let scan = provenance_scanner::scan_file(
        &repo.join("tracked.txt"),
        provenance_scanner::Language::Rust,
        "tracked\n",
    );
    assert_eq!(
        scan_commit(&repo, &[scan]).as_deref(),
        Some(expected.as_str())
    );
}

#[test]
fn a_scan_of_modified_files_has_no_commit_pin() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Utf8PathBuf::from_path_buf(dir.path().join("repo")).unwrap();
    run_git(["init", repo.as_str()]);
    std::fs::write(repo.join("tracked.rs"), "fn original() {}\n").unwrap();
    run_git(["-C", repo.as_str(), "add", "tracked.rs"]);
    run_git([
        "-C",
        repo.as_str(),
        "-c",
        "user.name=Provenance Test",
        "-c",
        "user.email=test@example.invalid",
        "commit",
        "-m",
        "fixture",
    ]);
    std::fs::write(repo.join("tracked.rs"), "fn changed() {}\n").unwrap();

    let scan = provenance_scanner::scan_file(
        &repo.join("tracked.rs"),
        provenance_scanner::Language::Rust,
        "fn changed() {}\n",
    );
    assert!(scan_commit(&repo, &[scan]).is_none());
}

#[test]
fn a_scan_of_ignored_source_files_has_no_commit_pin() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Utf8PathBuf::from_path_buf(dir.path().join("repo")).unwrap();
    run_git(["init", repo.as_str()]);
    std::fs::write(repo.join(".gitignore"), "ignored.rs\n").unwrap();
    run_git(["-C", repo.as_str(), "add", ".gitignore"]);
    run_git([
        "-C",
        repo.as_str(),
        "-c",
        "user.name=Provenance Test",
        "-c",
        "user.email=test@example.invalid",
        "commit",
        "-m",
        "fixture",
    ]);
    std::fs::write(repo.join("ignored.rs"), "fn ignored() {}\n").unwrap();
    let scan = provenance_scanner::scan_file(
        &repo.join("ignored.rs"),
        provenance_scanner::Language::Rust,
        "fn ignored() {}\n",
    );

    assert!(scan_commit(&repo, &[scan]).is_none());
}

fn run_git<const N: usize>(args: [&str; N]) {
    let output = std::process::Command::new("git")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_stdout<const N: usize>(args: [&str; N]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}
