//! A distributed binary has to be able to say which build it is.
//!
//! Implementation aid: no Rule in the graph owns build identity yet. These
//! tests pin the output that tells two builds of one release apart.

#[path = "review_host_support/mod.rs"]
mod review_host_support;

#[path = "../build/inputs.rs"]
mod inputs;

use assert_cmd::Command;
use std::path::Path;

/// The workspace checkout that holds this crate. The tests run in a checkout of
/// this repository.
fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
}

fn git(arguments: &[&str]) -> String {
    let output = provenance_store::git_process::git()
        .arg("-C")
        .arg(workspace_root())
        .args(arguments)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {arguments:?} failed");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
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
/// because the managed skill stamp and the release checks compare it. The build
/// then names the commit of this checkout. It adds `-dirty` exactly when git
/// reports a change to a tracked path in the build input list.
#[test]
fn version_flag_names_the_release_and_the_commit_of_the_build() {
    let release = env!("CARGO_PKG_VERSION");
    let head = git(&["rev-parse", "HEAD"]);
    let mut arguments = vec!["--literal-pathspecs", "diff", "--name-only", "HEAD", "--"];
    let inputs = inputs::paths(workspace_root());
    arguments.extend(inputs.iter().map(String::as_str));
    let changes = git(&arguments);
    let mark = if changes.is_empty() { "" } else { "-dirty" };
    let build = reported_build();
    assert_eq!(
        build,
        format!("{release} ({}{mark})", &head[..8]),
        "changed build inputs: {changes:?}"
    );
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
