use serde_json::Value;
use std::{env, fs, path::PathBuf, process::Command};

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
#[provenance_macros::verifies("rule_cargo_install_prints_init_step", examples)]
fn cargo_install_emits_the_exact_init_next_step_from_the_cli_build_script() {
    let workspace = workspace_root();
    let build_script = fs::read_to_string(workspace.join("crates/provenance-cli/build.rs"))
        .expect("read provenance-cli build script");
    let next_step = "Next step: run cargo provenance init in your project.";

    assert_eq!(build_script.matches(next_step).count(), 1);
    assert!(build_script.contains(&format!("cargo:warning={next_step}")));

    let manifest = fs::read_to_string(workspace.join("crates/provenance-cli/Cargo.toml")).unwrap();
    assert!(manifest.contains("homepage.workspace = true"));
}

#[test]
fn windows_cli_reserves_enough_main_thread_stack_for_argument_parsing() {
    let build_script = fs::read_to_string(workspace_root().join("crates/provenance-cli/build.rs"))
        .expect("read provenance-cli build script");

    assert!(build_script.contains("CARGO_CFG_TARGET_OS"));
    assert!(build_script.contains("CARGO_CFG_TARGET_ENV"));
    assert!(build_script.contains("cargo:rustc-link-arg-bin=provenance=/STACK:8388608"));
}

#[test]
fn ste_download_client_carries_no_quic_transport() {
    let workspace = workspace_root();
    let root_manifest = fs::read_to_string(workspace.join("Cargo.toml")).unwrap();
    let cli_manifest =
        fs::read_to_string(workspace.join("crates/provenance-cli/Cargo.toml")).unwrap();
    let lock = fs::read_to_string(workspace.join("Cargo.lock")).unwrap();

    assert!(root_manifest.contains("ureq ="));
    assert!(cli_manifest.contains("ureq.workspace = true"));
    assert!(!root_manifest.contains("reqwest ="));
    assert!(!cli_manifest.contains("reqwest.workspace = true"));
    assert!(!lock.contains("name = \"quinn-proto\""));
}

#[test]
fn init_download_work_runs_on_tokios_blocking_pool() {
    let workspace = workspace_root();
    let handlers = fs::read_to_string(workspace.join("crates/provenance-cli/src/handlers/mod.rs"))
        .expect("read command dispatcher");
    let onboarding =
        fs::read_to_string(workspace.join("crates/provenance-cli/src/ste_onboarding.rs"))
            .expect("read STE onboarding");

    assert!(handlers.matches("tokio::task::spawn_blocking").count() >= 2);
    assert!(!onboarding.contains("std::thread::spawn"));
}

#[test]
fn release_tests_keep_the_ste_asset_override_on_loopback() {
    let onboarding =
        fs::read_to_string(workspace_root().join("crates/provenance-cli/src/ste_onboarding.rs"))
            .expect("read STE onboarding");

    assert!(!onboarding.contains("cfg!(debug_assertions)"));
    assert!(onboarding.contains("127.0.0.1"));
    assert!(onboarding.contains("localhost"));
}

fn packed_install_job(workflow: &str) -> &str {
    workflow
        .split_once("  typescript-sdk-packed-install:")
        .unwrap()
        .1
        .split_once("  release-smoke-tools:")
        .unwrap()
        .0
}

fn packed_target_pairs(workflow: &str) -> Vec<(String, String)> {
    let matrix = packed_install_job(workflow)
        .split_once("        include:")
        .unwrap()
        .1
        .split_once("    steps:")
        .unwrap()
        .0;
    let mut targets = Vec::new();
    let mut runner = None;
    for line in matrix.lines().map(str::trim) {
        if let Some(value) = line.strip_prefix("- os: ") {
            assert!(runner.replace(value.to_owned()).is_none());
        } else if let Some(value) = line.strip_prefix("target: ") {
            targets.push((
                runner.take().expect("target follows its runner"),
                value.to_owned(),
            ));
        }
    }
    assert!(runner.is_none(), "each runner needs a target");
    targets
}

#[test]
fn packed_ste_gate_runs_on_every_supported_release_target() {
    let workspace = workspace_root();
    let targets: Value = serde_json::from_str(
        &fs::read_to_string(workspace.join(".github/release-targets.json")).unwrap(),
    )
    .unwrap();
    let workflow = fs::read_to_string(workspace.join(".github/workflows/ci.yml")).unwrap();
    let packed_job = packed_install_job(&workflow);

    let mut expected: Vec<_> = targets
        .as_array()
        .unwrap()
        .iter()
        .map(|target| {
            (
                target["smoke_os"].as_str().unwrap().to_owned(),
                target["target"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let mut actual = packed_target_pairs(&workflow);
    expected.sort();
    actual.sort();
    assert_eq!(actual, expected);
    assert!(packed_job.contains("npm run test:packed --prefix packages/provenance"));
}

#[test]
fn packed_target_parser_accepts_windows_line_endings() {
    let workflow = fs::read_to_string(workspace_root().join(".github/workflows/ci.yml")).unwrap();
    let windows_workflow = workflow.replace("\r\n", "\n").replace('\n', "\r\n");

    assert_eq!(
        packed_target_pairs(&windows_workflow),
        packed_target_pairs(&workflow)
    );
}

#[test]
fn pinned_actionlint_accepts_the_canonical_intel_runner() {
    let config =
        fs::read_to_string(workspace_root().join(".github/actionlint.yaml")).unwrap_or_default();

    assert!(
        config.contains("    - macos-15-intel"),
        "actionlint 1.7.7 needs the newer hosted runner in its configured label set"
    );
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
    assert_eq!(
        native
            .matches("release/cargo-provenance${{ matrix.executable_suffix }}")
            .count(),
        2
    );

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
    let targets: Value =
        serde_json::from_slice(&fs::read(workspace.join(".github/release-targets.json")).unwrap())
            .unwrap();
    let mut expected_platform_packages: Vec<_> = targets
        .as_array()
        .unwrap()
        .iter()
        .map(|target| target["npm"]["name"].as_str().unwrap())
        .collect();
    expected_platform_packages.sort_unstable();
    let mut platform_packages: Vec<_> = sdk["optionalDependencies"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    platform_packages.sort_unstable();
    assert_eq!(platform_packages, expected_platform_packages);
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
    let preflight = workflow
        .split_once("  preflight:")
        .unwrap()
        .1
        .split_once("  build:")
        .unwrap()
        .0;
    assert!(preflight.contains("verify-release-versions.sh"));
    assert!(preflight.contains(".github/release-targets.json"));
    let build_job = workflow
        .split_once("  build:")
        .unwrap()
        .1
        .split_once("  publish:")
        .unwrap()
        .0;
    assert!(build_job.contains("needs: preflight"));
    assert_eq!(workflow.matches("verify-release-versions.sh").count(), 1);
    let verifier = fs::read_to_string(&script).unwrap();
    assert!(verifier.contains(".github/release-targets.json"));
    assert!(!verifier.contains("@quality-sh/provenance-darwin-arm64"));

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

    for dependency in ["preflight", "publish-crates", "publish-npm"] {
        assert!(publish.contains(&format!("      - {dependency}")));
    }
}

#[test]
fn release_smoke_waits_for_preflight_and_both_registries() {
    let workflow = fs::read_to_string(workspace_root().join(".github/workflows/release.yml"))
        .expect("read release workflow");
    let smoke = workflow.split_once("  smoke:").unwrap().1;

    assert!(smoke.contains("      - preflight"));
    assert!(smoke.contains("      - publish"));
}

#[test]
fn initializer_smoke_covers_every_supported_package_manager() {
    let workflow = fs::read_to_string(workspace_root().join(".github/workflows/ci.yml")).unwrap();
    let manager_job = workflow
        .split_once("  initializer:")
        .unwrap()
        .1
        .split_once("  typescript-sdk-packed-install:")
        .unwrap()
        .0;

    let smoke_loop = manager_job
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("for manager in "))
        .expect("the initializer job loops over the package managers");
    for manager in ["npm", "pnpm", "yarn", "bun", "deno", "nub"] {
        assert!(smoke_loop.contains(manager), "{manager}");
    }
}

#[test]
fn post_release_smoke_controls_deno_against_the_current_engine() {
    let workspace = workspace_root();
    let workflow =
        fs::read_to_string(workspace.join(".github/workflows/release-smoke.yml")).unwrap();
    let script =
        fs::read_to_string(workspace.join(".github/scripts/release-smoke/deno-registry.sh"))
            .expect("read Deno release smoke script");

    assert!(workflow.contains("  deno-registry:"));
    assert!(workflow.contains("deno-registry.sh \"$VERSION\""));
    assert!(script.contains("@quality-sh/create-provenance@$version"));
    assert!(script.contains("npm:@quality-sh/provenance@${version}"));
    assert!(script.contains("assert_provenance_check"));
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
