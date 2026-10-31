//! Export reads only the manifest and the shards. An entry under
//! `.provenance/state` that is neither does not make export fail.
#![cfg(unix)]

use assert_cmd::Command;
use std::os::unix::fs::PermissionsExt;

fn provenance(args: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("provenance")
        .unwrap()
        .args(args)
        .assert()
}

fn export_succeeds(repo: &str) {
    let output = provenance(&[
        "export", "--repo", repo, "--scope", "default", "--format", "json",
    ])
    .success()
    .get_output()
    .stdout
    .clone();
    let _: serde_json::Value = serde_json::from_slice(&output).unwrap();
}

#[test]
fn export_leaves_out_stray_state_entries() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_string_lossy().to_string();
    provenance(&[
        "init",
        "--path",
        &repo,
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ])
    .success();
    let state = dir.path().join(".provenance/state");
    let scope = state.join("scopes/default/sources");
    std::fs::create_dir_all(&scope).unwrap();

    std::fs::write(scope.join("stray\\name.jsonl"), b"x").unwrap();
    export_succeeds(&repo);

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        let name = std::ffi::OsStr::from_bytes(b"stray-\xff.jsonl");
        std::fs::write(state.join(name), b"x").unwrap();
        export_succeeds(&repo);
    }

    let unreadable = scope.join("notes.txt");
    std::fs::write(&unreadable, b"x").unwrap();
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000)).unwrap();
    export_succeeds(&repo);
}
