use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::Value;

const CRATE_ORDER: [&str; 7] = [
    "provenance-macros",
    "provenance-core",
    "provenance-scanner",
    "provenance-ste100",
    "provenance-store",
    "provenance-sdk",
    "provenance-cli",
];

#[test]
fn rust_release_packages_have_registry_metadata_and_versioned_dependencies() {
    let workspace = workspace_root();
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(&workspace)
        .output()
        .expect("run cargo metadata");
    assert!(output.status.success(), "cargo metadata failed");
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("parse cargo metadata");
    let packages = metadata["packages"]
        .as_array()
        .expect("metadata packages are an array");

    for crate_name in CRATE_ORDER {
        let package = packages
            .iter()
            .find(|package| package["name"] == crate_name)
            .unwrap_or_else(|| panic!("metadata omitted {crate_name}"));
        let version = package["version"]
            .as_str()
            .expect("package version is text");

        assert_eq!(
            package["license"], "BUSL-1.1",
            "{crate_name} has no release license"
        );
        assert_eq!(
            package["repository"], "https://github.com/quality-sh/provenance",
            "{crate_name} has no canonical repository",
        );
        assert_eq!(
            package["homepage"], "https://github.com/quality-sh/provenance",
            "{crate_name} has no canonical homepage",
        );
        assert!(
            package["description"]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "{crate_name} has no description",
        );
        assert!(package["readme"].is_string(), "{crate_name} has no README");
        assert_eq!(
            package["publish"],
            serde_json::json!(["crates-io"]),
            "{crate_name} must publish only to crates.io",
        );

        for dependency in package["dependencies"]
            .as_array()
            .expect("package dependencies are an array")
        {
            let Some(dependency_name) = dependency["name"].as_str() else {
                continue;
            };
            if !CRATE_ORDER.contains(&dependency_name) {
                continue;
            }
            assert_eq!(
                dependency["req"],
                format!("^{version}"),
                "{crate_name} must give {dependency_name} the shared registry version",
            );
        }
    }
}

#[test]
fn release_workflow_publishes_rust_crates_in_dependency_order() {
    let workspace = workspace_root();
    let workflow = fs::read_to_string(workspace.join(".github/workflows/release.yml"))
        .expect("read release workflow");
    let mut previous = 0;
    for crate_name in CRATE_ORDER {
        let offset = workflow[previous..]
            .find(crate_name)
            .unwrap_or_else(|| panic!("release workflow omits {crate_name}"));
        previous += offset + crate_name.len();
    }
    assert!(workflow.contains("name: Publish Rust crates"));
    assert!(workflow.contains("environment: crates-io"));
    assert!(workflow.contains("publish-cargo-if-missing.sh"));
    assert!(workflow.contains("run: cargo package --workspace --locked"));
    assert!(workflow.contains("toolchain: 1.98.0"));
    assert!(!workflow.contains("run: cargo publish"));
}

#[test]
fn cli_package_contains_its_embedded_skills() {
    let workspace = workspace_root();
    let output = Command::new(env!("CARGO"))
        .args([
            "package",
            "--locked",
            "--no-verify",
            "--allow-dirty",
            "--package",
            "provenance-cli",
            "--list",
        ])
        .current_dir(workspace)
        .output()
        .expect("list the CLI package");
    assert!(
        output.status.success(),
        "cargo package failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    let files = String::from_utf8(output.stdout).expect("package file list is UTF-8");
    for skill in [
        "provenance-fork-tournament",
        "provenance-grounded-writing",
        "provenance-shaping",
        "provenance-swarm-backtrace",
    ] {
        assert!(
            files
                .lines()
                .any(|file| file == format!("skills/{skill}/SKILL.md")),
            "CLI package omits {skill}",
        );
    }
}

#[cfg(unix)]
mod publish_helper {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const LOCAL_CHECKSUM: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn an_existing_crate_version_is_not_published_again() {
        let result = invoke("200", LOCAL_CHECKSUM, "false");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let calls = fs::read_to_string(result.call_log).expect("read helper calls");
        assert!(calls.contains("curl "));
        assert!(calls.contains("/provenance-core/0.2.0"));
        assert!(!calls.contains("cargo "), "{calls}");
    }

    #[test]
    fn a_missing_crate_version_is_published() {
        let result = invoke("404", "unused-checksum", "false");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let calls = fs::read_to_string(result.call_log).expect("read helper calls");
        assert!(calls.contains("/provenance-core/0.2.0"));
        assert!(
            calls.contains(
                "cargo token=unset publish --registry crates-io --dry-run --locked --package provenance-core",
            ),
            "{calls}",
        );
        assert!(
            calls.contains(
                "cargo token=test-token publish --registry crates-io --no-verify --locked --package provenance-core",
            ),
            "{calls}",
        );
        assert!(
            calls.contains("cargo token=unset info --registry crates-io provenance-core@0.2.0",),
            "{calls}",
        );
    }

    #[test]
    fn an_unexpected_registry_response_fails_closed() {
        let result = invoke("503", "unused-checksum", "false");
        assert!(!result.status.success());
        let calls = fs::read_to_string(result.call_log).expect("read helper calls");
        assert!(!calls.contains("cargo "), "{calls}");
    }

    #[test]
    fn an_existing_version_with_different_contents_fails_closed() {
        let result = invoke(
            "200",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "false",
        );
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("checksum"),
            "{}",
            String::from_utf8_lossy(&result.stderr),
        );
        let calls = fs::read_to_string(result.call_log).expect("read helper calls");
        assert!(!calls.contains("cargo "), "{calls}");
    }

    #[test]
    fn a_yanked_existing_version_fails_closed() {
        let result = invoke("200", LOCAL_CHECKSUM, "true");
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("yanked"),
            "{}",
            String::from_utf8_lossy(&result.stderr),
        );
        let calls = fs::read_to_string(result.call_log).expect("read helper calls");
        assert!(!calls.contains("cargo "), "{calls}");
    }

    struct Invocation {
        status: std::process::ExitStatus,
        stderr: Vec<u8>,
        call_log: PathBuf,
        _temporary: tempfile::TempDir,
    }

    fn invoke(status_code: &str, registry_checksum: &str, yanked: &str) -> Invocation {
        let workspace = workspace_root();
        let temporary = tempfile::tempdir().expect("create helper directory");
        let call_log = temporary.path().join("calls.log");
        let archive = temporary.path().join("provenance-core-0.2.0.crate");
        fs::write(&archive, "test crate archive").expect("write fake crate archive");
        write_executable(
            &temporary.path().join("curl"),
            "#!/bin/sh\nprintf 'curl token=%s %s\\n' \"${CARGO_REGISTRY_TOKEN-unset}\" \"$*\" >> \"$PUBLISH_CALL_LOG\"\noutput=/dev/null\nwant_output=false\nfor argument in \"$@\"; do\n  if [ \"$want_output\" = true ]; then\n    output=$argument\n    want_output=false\n  elif [ \"$argument\" = --output ]; then\n    want_output=true\n  fi\ndone\nprintf '{\"version\":{\"checksum\":\"%s\",\"yanked\":%s}}' \"$FAKE_REGISTRY_CHECKSUM\" \"$FAKE_REGISTRY_YANKED\" > \"$output\"\nprintf '%s' \"$FAKE_CRATES_STATUS\"\n",
        );
        write_executable(
            &temporary.path().join("cargo"),
            "#!/bin/sh\nprintf 'cargo token=%s %s\\n' \"${CARGO_REGISTRY_TOKEN-unset}\" \"$*\" >> \"$PUBLISH_CALL_LOG\"\n",
        );
        write_executable(
            &temporary.path().join("sha256sum"),
            "#!/bin/sh\nprintf '%s  %s\\n' \"$FAKE_LOCAL_CHECKSUM\" \"$1\"\n",
        );
        let path = env::join_paths(
            std::iter::once(temporary.path().to_path_buf())
                .chain(env::split_paths(&env::var_os("PATH").expect("PATH is set"))),
        )
        .expect("build helper PATH");
        let output = Command::new("bash")
            .args([
                workspace
                    .join(".github/scripts/publish-cargo-if-missing.sh")
                    .to_str()
                    .expect("helper path is UTF-8"),
                "provenance-core",
                "0.2.0",
                archive.to_str().expect("archive path is UTF-8"),
            ])
            .env("PATH", path)
            .env("PUBLISH_CALL_LOG", &call_log)
            .env("FAKE_CRATES_STATUS", status_code)
            .env("FAKE_REGISTRY_CHECKSUM", registry_checksum)
            .env("FAKE_REGISTRY_YANKED", yanked)
            .env("FAKE_LOCAL_CHECKSUM", LOCAL_CHECKSUM)
            .env("CARGO_REGISTRY_TOKEN", "test-token")
            .current_dir(workspace)
            .output()
            .expect("run cargo publication helper");

        Invocation {
            status: output.status,
            stderr: output.stderr,
            call_log,
            _temporary: temporary,
        }
    }

    fn write_executable(path: &Path, contents: &str) {
        fs::write(path, contents).expect("write fake executable");
        let mut permissions = fs::metadata(path)
            .expect("read fake metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("make fake executable runnable");
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("CLI crate is nested under the workspace root")
        .to_path_buf()
}
