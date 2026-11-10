//! Runs the review asset build script in a small package with Cargo.
//!
//! Implementation aid: no Rule states when the build script runs. The test
//! keeps a build with no change from building the review crate again.
use std::path::Path;
use std::process::Command;

/// Builds the package and tells if Cargo ran its build script.
fn build_runs_script(package: &Path, target: &Path) -> bool {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["build", "--offline", "-v"])
        .current_dir(package)
        .env("CARGO_TARGET_DIR", target)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    stderr
        .lines()
        .any(|line| line.trim_start().starts_with("Running") && line.contains("build-script-build"))
}

#[test]
fn a_build_without_a_change_does_not_run_the_asset_script_again() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = tempfile::tempdir().unwrap();
    let package = directory.path().join("probe");
    let target = directory.path().join("target");
    std::fs::create_dir_all(package.join("build")).unwrap();
    std::fs::create_dir_all(package.join("src")).unwrap();
    std::fs::create_dir_all(package.join("review-assets")).unwrap();
    std::fs::copy(crate_root.join("build.rs"), package.join("build.rs")).unwrap();
    std::fs::copy(
        crate_root.join("build/review_assets.rs"),
        package.join("build/review_assets.rs"),
    )
    .unwrap();
    std::fs::write(
        package.join("Cargo.toml"),
        "[package]\nname = \"review-asset-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(
        package.join("src/lib.rs"),
        "include!(concat!(env!(\"OUT_DIR\"), \"/review_assets.rs\"));\n\
         pub fn count() -> usize { ASSETS.len() }\n",
    )
    .unwrap();
    std::fs::write(package.join("review-assets/index.html"), "<!doctype html>").unwrap();

    assert!(build_runs_script(&package, &target), "the first build");
    assert!(
        !build_runs_script(&package, &target),
        "a build with no change ran the asset script again"
    );

    let generated = package.join("review-assets-generated");
    std::fs::create_dir(&generated).unwrap();
    std::fs::write(generated.join("index.html"), "<!doctype html>").unwrap();
    assert!(
        build_runs_script(&package, &target),
        "the generated directory did not start the asset script"
    );
    assert!(!build_runs_script(&package, &target), "the build after it");
}
