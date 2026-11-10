//! TypeScript init commands where the command does not find the project
//! package: each command fails and does not run a `provenance` executable from
//! PATH.
#![cfg(unix)]

use provenance_macros::verifies;
use std::path::{Path, PathBuf};
use std::process::Output;

#[path = "cli_init_typescript/support.rs"]
#[allow(dead_code)]
mod support;

use support::{link_project_binary, on_path, shell, write_package_json, MANAGERS, PACKAGE_BIN};

/// When the project package is missing, the command of each package manager on
/// this host fails and does not run a `provenance` executable from PATH. A
/// command that names the bin file of the package also names that file in its
/// error, so a run of a different program that fails cannot pass. Each run has
/// a new, empty npm cache, so the result does not depend on the npm cache of
/// the host. The test puts no package in that cache.
#[test]
#[verifies("rule_init_typescript_local_command", examples)]
fn missing_package_command_does_not_select_a_provenance_executable_from_path() {
    for (manager, executable, prefix) in MANAGERS {
        if !on_path(executable) {
            eprintln!("{manager}: {executable} is not on PATH, so the test did not run it");
            continue;
        }
        let temporary = tempfile::tempdir().unwrap();
        let project = temporary.path().join("project");
        std::fs::create_dir(&project).unwrap();
        write_package_json(&project);

        let run = run_with_sentinel(temporary.path(), &project, prefix);

        assert_failed_without_sentinel(manager, &run);
        if prefix.ends_with(PACKAGE_BIN) {
            assert_names_missing_bin_file(manager, &run.output);
        }
        eprintln!("{manager}: the command failed and the PATH executable did not run");
    }
}

/// Each command that names the bin file of the package fails when it starts in
/// a subdirectory of a project that has the package installed. This test pins
/// the limit that the AGENTS.md item states: run the commands from the
/// directory that contains `node_modules`.
#[test]
#[verifies("rule_init_typescript_local_command", examples)]
fn bin_file_command_fails_from_a_project_subdirectory() {
    for (manager, executable, prefix) in MANAGERS {
        if !prefix.ends_with(PACKAGE_BIN) {
            continue;
        }
        if !on_path(executable) {
            eprintln!("{manager}: {executable} is not on PATH, so the test did not run it");
            continue;
        }
        let temporary = tempfile::tempdir().unwrap();
        let project = temporary.path().join("project");
        std::fs::create_dir(&project).unwrap();
        write_package_json(&project);
        link_project_binary(&project);
        let subdirectory = project.join("sub/deep");
        std::fs::create_dir_all(&subdirectory).unwrap();

        let run = run_with_sentinel(temporary.path(), &subdirectory, prefix);

        assert_failed_without_sentinel(manager, &run);
        assert_names_missing_bin_file(manager, &run.output);
        eprintln!("{manager}: the command failed in a subdirectory");
    }
}

struct SentinelRun {
    output: Output,
    selected: PathBuf,
}

/// Runs `<prefix> --version` in `directory` with a sentinel `provenance` first
/// on PATH and a new, empty npm cache. The sentinel writes a marker file and
/// exits 42.
fn run_with_sentinel(temporary: &Path, directory: &Path, prefix: &str) -> SentinelRun {
    use std::os::unix::fs::PermissionsExt;

    let sentinel_directory = temporary.join("sentinel");
    std::fs::create_dir(&sentinel_directory).unwrap();
    let selected = temporary.join("selected");
    let sentinel = sentinel_directory.join("provenance");
    std::fs::write(
        &sentinel,
        format!(
            "#!/bin/sh\nprintf selected > '{}'\nexit 42\n",
            selected.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&sentinel, std::fs::Permissions::from_mode(0o755)).unwrap();
    let cache = temporary.join("npm-cache");
    std::fs::create_dir(&cache).unwrap();
    let path = format!(
        "{}:{}",
        sentinel_directory.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let output = shell(directory, &format!("{prefix} --version"))
        .env("PATH", path)
        .env("npm_config_cache", &cache)
        .output()
        .unwrap();
    SentinelRun { output, selected }
}

fn assert_failed_without_sentinel(manager: &str, run: &SentinelRun) {
    assert!(
        !run.output.status.success(),
        "{manager}: {}",
        String::from_utf8_lossy(&run.output.stdout)
    );
    assert!(!run.selected.exists(), "{manager}: the PATH executable ran");
}

fn assert_names_missing_bin_file(manager: &str, output: &Output) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(PACKAGE_BIN), "{manager}: {stderr}");
}
