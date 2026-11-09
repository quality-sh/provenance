//! A distributed binary has to be able to say which build it is.
//!
//! Implementation aid: no Rule in the graph owns build identity yet. These
//! tests pin the output that tells two builds of one release apart.

#[path = "review_host_support/mod.rs"]
mod review_host_support;

use assert_cmd::Command;
use std::path::Path;

/// The commit of this checkout, or nothing when the tests do not run in a git
/// checkout of this repository, as in a package build from crates.io.
fn checkout_commit() -> Option<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.parent()?;
    let git = |arguments: &[&str]| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(arguments)
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8(output.stdout).unwrap().trim().to_owned())
    };
    let top = git(&["rev-parse", "--show-toplevel"])?;
    if Path::new(&top).canonicalize().ok()? != root.canonicalize().ok()? {
        return None;
    }
    Some(git(&["rev-parse", "HEAD"])?[..8].to_owned())
}

/// The text after the binary name in `provenance --version`.
fn reported_build() -> String {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    stdout
        .strip_prefix("provenance ")
        .and_then(|rest| rest.strip_suffix('\n'))
        .unwrap_or_else(|| panic!("unexpected version output {stdout:?}"))
        .to_owned()
}

/// Implementation aid: the release number stays the first token after the name,
/// because the managed skill stamp and the release checks compare it. A build
/// from a git checkout then names its commit, with `-dirty` when tracked files
/// had changes.
#[test]
fn version_flag_names_the_release_and_the_commit_of_the_build() {
    let release = env!("CARGO_PKG_VERSION");
    let build = reported_build();
    match checkout_commit() {
        Some(commit) => assert!(
            build == format!("{release} ({commit})")
                || build == format!("{release} ({commit}-dirty)"),
            "build {build:?} does not name release {release} and commit {commit}"
        ),
        None => assert_eq!(build, release),
    }
    let short = Command::cargo_bin("provenance")
        .unwrap()
        .arg("-V")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(short.stdout).unwrap(),
        format!("provenance {build}\n")
    );
}

/// Implementation aid: a long-running review host tells which build serves it,
/// with the same identity as `provenance --version`, next to its existing start
/// fields.
#[test]
fn review_host_start_line_carries_the_build_identity() {
    let repo = review_host_support::repository();
    let host = review_host_support::start(repo.path());
    assert_eq!(host.config["build"], reported_build());
    for field in [
        "endpoint",
        "bearer",
        "repositoryId",
        "scope",
        "instanceNonce",
        "url",
    ] {
        assert!(host.config[field].is_string(), "start line lost {field}");
    }
}
