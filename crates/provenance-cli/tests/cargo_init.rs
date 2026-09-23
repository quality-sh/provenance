use assert_cmd::Command;
use predicates::prelude::*;
use provenance_macros::verifies;
use serde_json::json;
use std::path::Path;

#[path = "cargo_init/support.rs"]
mod support;
use support::CargoFixture;

#[test]
#[verifies("rule_cargo_init_selects_workspace_package", examples)]
fn cargo_injected_argument_initializes_one_package_at_the_cli_version() {
    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);

    fixture
        .command()
        .arg("provenance")
        .arg("init")
        .assert()
        .success();

    assert_eq!(fixture.scope_path_prefix(), ".");
    assert_eq!(fixture.gitignore(), ".provenance/cache/\n");
    assert!(fixture.root().join("AGENTS.md").exists());
    assert!(fixture
        .root()
        .join(".agents/skills/provenance-shaping/SKILL.md")
        .exists());
    assert!(fixture
        .cargo_calls()
        .contains("add provenance-sdk@=0.2.3 --manifest-path"));
}

#[test]
fn initializer_is_idempotent() {
    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);
    fixture
        .command()
        .arg("provenance")
        .arg("init")
        .assert()
        .success();
    let manifest = fixture.manifest();
    let cargo_manifest = fixture.cargo_manifest();
    let cargo_lock = fixture.cargo_lock();

    fixture
        .command()
        .arg("provenance")
        .arg("init")
        .assert()
        .success();

    assert_eq!(fixture.manifest(), manifest);
    assert_eq!(fixture.gitignore(), ".provenance/cache/\n");
    assert_eq!(fixture.cargo_manifest(), cargo_manifest);
    assert_eq!(fixture.cargo_lock(), cargo_lock);
    assert_eq!(
        fixture
            .cargo_calls()
            .lines()
            .filter(|call| call.starts_with("add "))
            .count(),
        1
    );
}

#[test]
fn cargo_and_plain_init_write_identical_onboarding_files() {
    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);
    fixture
        .command()
        .args(["provenance", "init"])
        .assert()
        .success();
    let plain = fixture.temporary.path().join("plain");
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            plain.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();

    for relative in [
        ".provenance/state/manifest.json",
        ".gitignore",
        "AGENTS.md",
        ".agents/skills/provenance-fork-tournament/SKILL.md",
        ".agents/skills/provenance-grounded-writing/SKILL.md",
        ".agents/skills/provenance-shaping/SKILL.md",
        ".agents/skills/provenance-swarm-backtrace/SKILL.md",
    ] {
        assert_eq!(
            std::fs::read(fixture.root().join(relative)).unwrap(),
            std::fs::read(plain.join(relative)).unwrap(),
            "onboarding output differs at {relative}"
        );
    }
}

#[test]
#[verifies("rule_cargo_init_selects_workspace_package", examples)]
fn ambiguous_workspace_requires_a_package() {
    let fixture = CargoFixture::new(&[
        ("api", "crates/api/Cargo.toml"),
        ("worker", "crates/worker/Cargo.toml"),
    ]);

    fixture
        .command()
        .arg("provenance")
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("more than one eligible package"))
        .stderr(predicate::str::contains("--package"))
        .stderr(predicate::str::contains("api"))
        .stderr(predicate::str::contains("worker"));

    assert!(!fixture.root().join(".provenance").exists());
    assert!(!fixture.cargo_calls().contains("add "));
}

#[test]
#[verifies("rule_init_plans_all_project_writes", examples)]
fn repository_planning_failure_does_not_run_cargo_add() {
    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);
    let skill = fixture
        .root()
        .join(".agents/skills/provenance-shaping/SKILL.md");
    std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
    std::fs::write(&skill, "user-owned skill\n").unwrap();

    fixture
        .command()
        .args(["provenance", "init"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("exists and differs"));

    assert!(!fixture.cargo_calls().contains("add "));
}

#[test]
#[verifies("rule_cargo_init_restores_owned_files", examples)]
fn a_failing_cargo_add_restores_its_partial_manifest_and_lock_changes() {
    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);
    let manifest = fixture.cargo_manifest();

    fixture
        .command()
        .env("FAKE_CARGO_FAIL_AFTER_WRITE", "1")
        .args(["provenance", "init"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cargo add"));

    assert_eq!(fixture.cargo_manifest(), manifest);
    assert!(!fixture.root().join("Cargo.lock").exists());
}

#[test]
fn post_command_observation_failure_restores_the_file_it_proved_owned() {
    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);
    let manifest = fixture.cargo_manifest();

    fixture
        .command()
        .env("FAKE_CARGO_BREAK_LOCK_OBSERVATION", "1")
        .args(["provenance", "init"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "restored Cargo.toml after post-command observation failed",
        ));

    assert_eq!(fixture.cargo_manifest(), manifest);
    assert!(fixture.root().join("Cargo.lock").is_dir());
}

#[cfg(unix)]
#[test]
#[verifies("rule_cargo_init_restores_owned_files", examples)]
fn later_init_failure_exactly_restores_cargo_files() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);
    let manifest = std::fs::read(&fixture.cargo_manifest).unwrap();
    std::fs::set_permissions(
        &fixture.cargo_manifest,
        std::fs::Permissions::from_mode(0o640),
    )
    .unwrap();
    fixture
        .command()
        .env("FAKE_CARGO_STALE_INIT_PLAN", "1")
        .args(["provenance", "init"])
        .assert()
        .failure()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains(
            "changed after initialization was planned",
        ));

    assert_eq!(std::fs::read(&fixture.cargo_manifest).unwrap(), manifest);
    assert_eq!(
        std::fs::metadata(&fixture.cargo_manifest).unwrap().mode() & 0o777,
        0o640
    );
    assert!(!fixture.root().join("Cargo.lock").exists());
}

#[cfg(unix)]
#[test]
#[verifies("rule_cargo_init_restores_owned_files", examples)]
fn failed_lock_restore_compensates_the_manifest_restore() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = CargoFixture::new(&[("app", "crates/app/Cargo.toml")]);
    let assertion = fixture
        .command()
        .env("FAKE_CARGO_ROLLBACK_SECOND_RESTORE_FAIL", "1")
        .args(["provenance", "init"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cargo rollback failed"));

    std::fs::set_permissions(fixture.root(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assertion.stderr(predicate::str::contains("failed to restore"));
    assert!(fixture.cargo_manifest().contains("provenance-sdk"));
    assert_eq!(fixture.cargo_lock(), "version = 4\n");
}

#[cfg(unix)]
#[test]
#[verifies("rule_cargo_init_restores_owned_files", examples)]
fn failed_manifest_restore_leaves_both_post_cargo_files_untouched() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = CargoFixture::new(&[("app", "crates/app/Cargo.toml")]);
    fixture
        .command()
        .env("FAKE_CARGO_ROLLBACK_FIRST_RESTORE_FAIL", "1")
        .args(["provenance", "init"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cargo rollback failed"))
        .stderr(predicate::str::contains("failed to restore"));

    std::fs::set_permissions(
        fixture.root().join("crates/app"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    assert!(fixture.cargo_manifest().contains("provenance-sdk"));
    assert_eq!(fixture.cargo_lock(), "version = 4\n");
}

#[test]
#[verifies("rule_cargo_init_uses_package_directory", examples)]
fn explicit_package_selects_its_manifest_directory_as_the_path_prefix() {
    let fixture = CargoFixture::new(&[
        ("api", "crates/api/Cargo.toml"),
        ("worker", "crates/worker/Cargo.toml"),
    ]);

    fixture
        .command()
        .args(["provenance", "init", "--package", "api"])
        .assert()
        .success();

    assert_eq!(fixture.scope_path_prefix(), "crates/api");
    assert!(fixture
        .cargo_calls()
        .contains("add provenance-sdk@=0.2.3 --manifest-path"));
}

#[test]
#[verifies("rule_cargo_init_mutates_selected_manifest", examples)]
fn cargo_add_targets_the_selected_manifest_without_resolving_its_name_again() {
    let fixture = CargoFixture::new(&[
        ("api", "crates/api/Cargo.toml"),
        ("worker", "crates/worker/Cargo.toml"),
    ]);

    fixture
        .command()
        .args(["provenance", "init", "--package", "api"])
        .assert()
        .success();

    let add = fixture
        .cargo_calls()
        .lines()
        .find(|call| call.starts_with("add "))
        .unwrap()
        .to_owned();
    let (_, manifest) = add.rsplit_once(" --manifest-path ").unwrap();
    assert_eq!(
        Path::new(manifest),
        fixture.root().join("crates").join("api").join("Cargo.toml")
    );
    assert!(!add.contains(" --package "), "{add}");
}

#[test]
fn rerun_refuses_to_change_the_existing_target_package() {
    let fixture = CargoFixture::new(&[
        ("api", "crates/api/Cargo.toml"),
        ("worker", "crates/worker/Cargo.toml"),
    ]);
    fixture
        .command()
        .args(["provenance", "init", "--package", "api"])
        .assert()
        .success();

    fixture
        .command()
        .args(["provenance", "init", "--package", "worker"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already targets Cargo package"))
        .stderr(predicate::str::contains("api"))
        .stderr(predicate::str::contains("worker"));

    assert_eq!(fixture.scope_path_prefix(), "crates/api");
    assert!(!fixture
        .cargo_calls()
        .contains("add provenance-sdk@0.2.3 --package worker --exact"));
}

#[test]
fn rerun_accepts_an_equivalent_existing_path_prefix() {
    let fixture = CargoFixture::new(&[("api", "crates/api/Cargo.toml")]);
    fixture
        .command()
        .args(["provenance", "init"])
        .assert()
        .success();
    let mut manifest = fixture.manifest();
    manifest["scopes"][0]["path_prefix"] = json!("./crates/api");
    std::fs::write(
        fixture.root().join(".provenance/state/manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    fixture
        .command()
        .args(["provenance", "init"])
        .assert()
        .success();

    assert_eq!(fixture.scope_path_prefix(), "crates/api");
}

#[test]
fn cargo_init_reports_only_published_changes_then_no_change() {
    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);
    let assets = fixture.temporary.path().join("assets");
    let indexes = fixture.temporary.path().join("indexes");
    let first = fixture
        .command()
        .args(["provenance", "init"])
        .env("PROVENANCE_STE100_ASSET_DIR", &assets)
        .env("PROVENANCE_STE100_INDEX_DIR", &indexes)
        .env(
            "PROVENANCE_TEST_STE100_ASSET_URL",
            "http://127.0.0.1:9/unavailable",
        )
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first = String::from_utf8(first.stdout).unwrap();
    assert!(first.contains("  Cargo.toml (added the Provenance SDK dependency)\n"));
    assert!(first.contains("  Cargo.lock (created by Cargo)\n"));
    assert!(first.contains("  .provenance/state (manifest for scope \"default\")\n"));
    assert!(!first.contains("Dictionary:"));
    assert!(first.ends_with("Have your agent run provenance prime to get acclimated.\n"));
    let manifest = std::fs::read(fixture.root().join(".provenance/state/manifest.json")).unwrap();
    let cargo_manifest = fixture.cargo_manifest();
    let cargo_lock = fixture.cargo_lock();

    let second = fixture
        .command()
        .args(["provenance", "init"])
        .env("PROVENANCE_STE100_ASSET_DIR", &assets)
        .env("PROVENANCE_STE100_INDEX_DIR", &indexes)
        .env(
            "PROVENANCE_TEST_STE100_ASSET_URL",
            "http://127.0.0.1:9/unavailable",
        )
        .output()
        .unwrap();
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second = String::from_utf8(second.stdout).unwrap();
    assert!(second.contains("No change.\n"));
    assert!(!second.contains("\nNew\n"));
    assert!(!second.contains("\nChanged\n"));
    assert!(second.ends_with("Have your agent run provenance prime to get acclimated.\n"));
    assert_eq!(
        std::fs::read(fixture.root().join(".provenance/state/manifest.json")).unwrap(),
        manifest
    );
    assert_eq!(fixture.cargo_manifest(), cargo_manifest);
    assert_eq!(fixture.cargo_lock(), cargo_lock);
}
