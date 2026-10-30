#![cfg(windows)]

use assert_cmd::Command;

/// `provenance init` writes the manifest, the skill files and the onboarding
/// files with a temporary file and a rename. At a repository root of 240
/// characters or more, these paths are longer than the 260 characters that a
/// Windows file call accepts without the `\\?\` prefix.
#[test]
fn init_under_a_long_windows_root_writes_the_manifest() {
    let temp = tempfile::tempdir().unwrap();
    let mut repo = temp.path().to_path_buf();
    while repo.as_os_str().len() < 240 {
        repo.push("d".repeat(50));
    }

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();

    assert!(repo.join(".provenance/state/manifest.json").is_file());
}
