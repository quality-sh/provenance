use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::process::{Command, Output};

#[test]
fn release_smoke_workflow_has_exact_version_triggers_and_channel_jobs() {
    let workflow = fs::read_to_string(workspace_root().join(".github/workflows/release-smoke.yml"))
        .expect("read release smoke workflow");

    assert!(workflow.contains("release:"));
    assert!(workflow.contains("types: [published]"));
    assert!(workflow.contains("workflow_run:"));
    assert!(workflow.contains("workflows: [Release]"));
    assert!(workflow.contains("github.event.workflow_run.conclusion == 'success'"));
    assert!(workflow.contains("workflow_dispatch:"));
    assert!(workflow.contains("version:"));
    assert!(workflow.contains("required: true"));
    assert!(workflow.contains("name: crates.io installation"));
    assert!(workflow.contains("name: npm installation"));
    assert!(workflow.contains("name: GitHub archive (${{ matrix.target }})"));
    assert!(workflow.contains("fail-fast: false"));

    for target in [
        "x86_64-unknown-linux-gnu",
        "x86_64-pc-windows-msvc",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
    ] {
        assert!(workflow.contains(target), "workflow omits {target}");
    }

    for script in ["cargo-registry.sh", "npm-registry.sh", "github-archive.sh"] {
        assert!(
            workflow.contains(&format!(".github/scripts/release-smoke/{script}")),
            "workflow does not call {script}",
        );
    }

    let scripts = workspace_root().join(".github/scripts/release-smoke");
    let library = fs::read_to_string(scripts.join("lib.sh")).expect("read release smoke library");
    assert!(
        !library.contains(",,"),
        "library must support macOS Bash 3.2"
    );
    assert!(library.contains("assert_binary_version"));
    assert!(library.contains("cygpath -u"));

    let cargo =
        fs::read_to_string(scripts.join("cargo-registry.sh")).expect("read crates.io smoke script");
    assert!(cargo.contains("export CARGO_HOME=\"$smoke_root/cargo-home\""));
    assert!(cargo.contains("cargo install --locked provenance-cli --version \"=$version\""));
    assert!(cargo.contains("for binary in provenance cargo-provenance"));
    assert_eq!(
        cargo
            .lines()
            .filter(|line| line.trim() == "cargo provenance init")
            .count(),
        2
    );
    assert!(cargo.contains("use provenance_sdk::{rule, verifies};"));
    assert!(cargo.contains("cargo check --manifest-path"));
    assert!(cargo.contains("assert_binary_version"));

    let npm = fs::read_to_string(scripts.join("npm-registry.sh")).expect("read npm smoke script");
    assert!(npm.contains("export HOME=\"$smoke_root/home\""));
    assert!(npm.contains("export npm_config_cache=\"$smoke_root/npm-cache\""));
    assert!(npm.contains("@quality-sh/create-provenance@$version"));
    assert!(npm.contains("@quality-sh/provenance@$version"));
    assert!(npm.contains(".provenance/state/manifest.json"));
    assert!(npm.contains("retry_channel \"$channel\" initialize_npm_fixture"));
    assert!(npm.contains("assert_binary_version"));

    let archive = fs::read_to_string(scripts.join("github-archive.sh"))
        .expect("read GitHub archive smoke script");
    assert!(archive.contains("releases/download/$tag"));
    assert!(archive.contains("expected_checksum"));
    assert!(archive.contains("sha256_file"));
    assert!(archive.contains("for binary in provenance cargo-provenance"));
    assert!(archive.contains("assert_binary_version"));
}

#[cfg(unix)]
#[test]
fn exact_release_version_validation_rejects_tags_and_shell_text() {
    for version in ["0.2.2", "1.0.0-rc.1", "2.4.6+build.9"] {
        let output = call_library("validate_version \"$1\"", &[version], &[]);
        assert!(
            output.status.success(),
            "{version} was rejected: {}",
            String::from_utf8_lossy(&output.stderr),
        );
    }

    for version in ["v0.2.2", "0.2", "latest", "0.2.2; true", "0.2.2 rc1"] {
        let output = call_library("validate_version \"$1\"", &[version], &[]);
        assert!(!output.status.success(), "{version} was accepted");
    }
}

#[cfg(unix)]
#[test]
fn retry_stops_at_the_bound_and_reports_the_channel() {
    let temporary = tempfile::tempdir().expect("create retry fixture");
    let attempts = temporary.path().join("attempts");
    let worker = temporary.path().join("always-fail.sh");
    fs::write(
        &worker,
        "#!/usr/bin/env bash\nset -eu\ncount=0\nif [[ -f \"$ATTEMPTS_FILE\" ]]; then count=$(<\"$ATTEMPTS_FILE\"); fi\nprintf '%s\\n' \"$((count + 1))\" > \"$ATTEMPTS_FILE\"\nexit 9\n",
    )
    .expect("write retry worker");

    let output = call_library(
        "retry_channel crates.io bash \"$1\"",
        &[worker.to_str().expect("worker path is UTF-8")],
        &[
            (
                "ATTEMPTS_FILE",
                attempts.to_str().expect("attempt path is UTF-8"),
            ),
            ("RELEASE_SMOKE_RETRY_ATTEMPTS", "3"),
            ("RELEASE_SMOKE_RETRY_DELAY_SECONDS", "0"),
        ],
    );

    assert!(!output.status.success());
    assert_eq!(
        fs::read_to_string(attempts).expect("read attempts").trim(),
        "3"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("crates.io: failed after 3 attempts"),
        "{}",
        String::from_utf8_lossy(&output.stderr),
    );
}

#[cfg(unix)]
#[test]
fn checksum_lookup_requires_one_exact_archive_entry() {
    let temporary = tempfile::tempdir().expect("create checksum fixture");
    let checksums = temporary.path().join("SHA256SUMS");
    let digest = "a".repeat(64);
    fs::write(
        &checksums,
        format!(
            "{digest}  provenance-v0.2.2-target.tar.gz\n{}  other.tar.gz\n",
            "b".repeat(64)
        ),
    )
    .expect("write checksums");

    let output = call_library(
        "expected_checksum \"$1\" \"$2\"",
        &[
            checksums.to_str().expect("checksum path is UTF-8"),
            "provenance-v0.2.2-target.tar.gz",
        ],
        &[],
    );
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), digest);

    fs::write(
        &checksums,
        format!("{digest}  duplicate.zip\n{digest}  duplicate.zip\n"),
    )
    .expect("write duplicate checksums");
    let duplicate = call_library(
        "expected_checksum \"$1\" \"$2\"",
        &[
            checksums.to_str().expect("checksum path is UTF-8"),
            "duplicate.zip",
        ],
        &[],
    );
    assert!(!duplicate.status.success());
}

#[cfg(unix)]
#[test]
fn binary_version_check_requires_the_exact_reported_version() {
    let temporary = tempfile::tempdir().expect("create binary fixture");
    let binary = temporary.path().join("provenance");
    fs::write(
        &binary,
        "#!/usr/bin/env bash\nprintf 'provenance 10.2.20\\n'\n",
    )
    .expect("write fake binary");
    let mut permissions = fs::metadata(&binary)
        .expect("read fake binary metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&binary, permissions).expect("make fake binary executable");

    let output = call_library(
        "assert_binary_version crates.io 0.2.2 \"$1\"",
        &[binary.to_str().expect("binary path is UTF-8")],
        &[],
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("version other than 0.2.2"),
        "{}",
        String::from_utf8_lossy(&output.stderr),
    );
}

#[cfg(unix)]
fn call_library(command: &str, arguments: &[&str], environment: &[(&str, &str)]) -> Output {
    let library = workspace_root().join(".github/scripts/release-smoke/lib.sh");
    let mut invocation = Command::new("bash");
    invocation
        .args([
            "-c",
            &format!("source \"$0\"; {command}"),
            library.to_str().expect("library path is UTF-8"),
        ])
        .args(arguments);
    for (name, value) in environment {
        invocation.env(name, value);
    }
    invocation.output().expect("invoke release smoke library")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("CLI crate is nested under the workspace root")
        .to_path_buf()
}
