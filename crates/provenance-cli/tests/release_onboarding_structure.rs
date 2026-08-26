use serde_json::Value;
use std::{env, fs, path::PathBuf, process::Command};

const PLATFORM_PACKAGES: [&str; 4] = [
    "@quality-sh/provenance-darwin-arm64",
    "@quality-sh/provenance-darwin-x64",
    "@quality-sh/provenance-linux-x64-gnu",
    "@quality-sh/provenance-win32-x64-msvc",
];

#[test]
fn cli_package_ships_both_native_entry_points() {
    let metadata = cargo_metadata();
    let cli = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "provenance-cli")
        .expect("metadata includes provenance-cli");
    let mut binaries: Vec<_> = cli["targets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|target| {
            target["kind"]
                .as_array()
                .unwrap()
                .iter()
                .any(|kind| kind == "bin")
        })
        .map(|target| target["name"].as_str().unwrap())
        .collect();
    binaries.sort_unstable();

    assert_eq!(binaries, ["cargo-provenance", "provenance"]);
}

#[test]
fn cargo_provenance_is_a_std_only_forwarding_shim() {
    let workspace = workspace_root();
    let crate_root = workspace.join("crates/provenance-cli");
    let temporary = tempfile::tempdir().unwrap();
    let output = Command::new(env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2021", "-o"])
        .arg(temporary.path().join("cargo-provenance"))
        .arg(crate_root.join("src/bin/cargo-provenance.rs"))
        .env("CARGO_PKG_VERSION", env!("CARGO_PKG_VERSION"))
        .output()
        .expect("compile cargo-provenance directly");

    assert!(!crate_root.join("src/lib.rs").exists());
    assert!(
        output.status.success(),
        "the shim requires more than std: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_archives_include_cargo_provenance_but_npm_engines_do_not() {
    let workspace = workspace_root();
    let workflow = fs::read_to_string(workspace.join(".github/workflows/release.yml"))
        .expect("read release workflow");
    let native = workflow
        .split_once("- name: Package Unix archive")
        .unwrap()
        .1
        .split_once("- name: Package npm engine")
        .unwrap()
        .0;
    assert!(native.contains("release/cargo-provenance\""));
    assert!(native.contains("release/cargo-provenance.exe\""));

    let npm_script =
        fs::read_to_string(workspace.join("packages/provenance/scripts/package-engine.js"))
            .expect("read npm engine packager");
    assert!(!npm_script.contains("cargo-provenance"));
}

#[test]
fn workspace_release_versions_are_unified_at_0_2_2() {
    let metadata = cargo_metadata();
    assert!(metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .all(|package| package["version"] == "0.2.2"));

    let workspace = workspace_root();
    let sdk: Value = serde_json::from_slice(
        &fs::read(workspace.join("packages/provenance/package.json")).unwrap(),
    )
    .unwrap();
    let initializer: Value = serde_json::from_slice(
        &fs::read(workspace.join("packages/create-provenance/package.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(sdk["version"], "0.2.2");
    assert_eq!(initializer["version"], "0.2.2");
    let mut platform_packages: Vec<_> = sdk["optionalDependencies"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    platform_packages.sort_unstable();
    assert_eq!(platform_packages, PLATFORM_PACKAGES);
    assert!(sdk["optionalDependencies"]
        .as_object()
        .unwrap()
        .values()
        .all(|version| version == "0.2.2"));
}

#[test]
fn release_version_preflight_gates_every_artifact_build() {
    let workspace = workspace_root();
    let workflow = fs::read_to_string(workspace.join(".github/workflows/release.yml"))
        .expect("read release workflow");
    let script = workspace.join(".github/scripts/verify-release-versions.sh");

    assert!(
        script.exists(),
        "release version preflight script is missing"
    );
    assert!(workflow.contains("verify-versions:"));
    let build_job = workflow
        .split_once("  build:")
        .unwrap()
        .1
        .split_once("  publish:")
        .unwrap()
        .0;
    assert!(build_job.contains("needs: verify-versions"));
    assert_eq!(workflow.matches("verify-release-versions.sh").count(), 1);

    #[cfg(unix)]
    {
        let output = Command::new("bash")
            .arg(script)
            .arg("v0.2.2")
            .current_dir(workspace)
            .output()
            .expect("run release version preflight");
        assert!(
            output.status.success(),
            "release version preflight failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn github_release_waits_for_registry_publication() {
    let workflow = fs::read_to_string(workspace_root().join(".github/workflows/release.yml"))
        .expect("read release workflow");
    let publish = workflow
        .split_once("  publish:")
        .unwrap()
        .1
        .split_once("  publish-crates:")
        .unwrap()
        .0;

    assert!(publish.contains("needs: [publish-crates, publish-npm]"));
}

fn cargo_metadata() -> Value {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(workspace_root())
        .output()
        .expect("run cargo metadata");
    assert!(output.status.success(), "cargo metadata failed");
    serde_json::from_slice(&output.stdout).expect("parse cargo metadata")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("CLI crate is nested under the workspace root")
        .to_path_buf()
}
