use assert_cmd::Command;
use predicates::prelude::*;
use provenance_macros::verifies;
use serde_json::json;

#[path = "cargo_init/support.rs"]
mod support;
use support::CargoFixture;

#[test]
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
        .contains("add provenance-sdk@=0.2.2 --package app"));
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
fn repository_validation_failure_prevents_a_concurrent_cargo_edit() {
    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);
    fixture
        .command()
        .env("FAKE_CARGO_CONCURRENT_EDIT", "1")
        .args(["provenance", "init"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("neither file was restored"));

    assert_eq!(fixture.cargo_manifest(), "concurrent manifest\n");
    assert!(fixture.root().join("Cargo.lock").exists());
    assert!(fixture.cargo_calls().contains("add "));
}

#[cfg(unix)]
#[test]
#[verifies("rule_cargo_init_restores_owned_files", examples)]
fn concurrent_lock_edit_prevents_restoring_either_cargo_file() {
    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);
    fixture
        .command()
        .env("FAKE_CARGO_CONCURRENT_LOCK_EDIT", "1")
        .args(["provenance", "init"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("neither file was restored"));

    assert!(fixture.cargo_manifest().contains("provenance-sdk"));
    assert_eq!(fixture.cargo_lock(), "concurrent lock\n");
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
        .contains("add provenance-sdk@=0.2.2 --package api"));
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
        .contains("add provenance-sdk@0.2.2 --package worker --exact"));
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
