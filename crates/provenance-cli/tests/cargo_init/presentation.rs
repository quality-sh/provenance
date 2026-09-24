use super::CargoFixture;

#[cfg(target_os = "linux")]
#[test]
fn cargo_init_reports_a_failed_summary_after_publishing_state() {
    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);
    let output = fixture
        .std_command()
        .args(["provenance", "init"])
        .stdout(std::fs::File::create("/dev/full").unwrap())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No space left on device"));
    assert!(fixture
        .root()
        .join(".provenance/state/manifest.json")
        .is_file());
}

#[test]
fn noisy_cargo_add_does_not_print_during_quiet_success() {
    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);
    let output = fixture
        .command()
        .args(["provenance", "init", "--quiet"])
        .env("FAKE_CARGO_ADD_NOISE", "1")
        .env(
            "PROVENANCE_STE100_ASSET_DIR",
            fixture.temporary.path().join("assets"),
        )
        .env(
            "PROVENANCE_STE100_INDEX_DIR",
            fixture.temporary.path().join("indexes"),
        )
        .env(
            "PROVENANCE_TEST_STE100_ASSET_URL",
            "http://127.0.0.1:9/unavailable",
        )
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty(), "{:?}", output.stdout);
    let warning = String::from_utf8(output.stderr).unwrap();
    assert!(warning.contains("Warning: the official Issue 9 dictionary could not be added"));
    assert!(warning.contains("request the official Issue 9 PDF"));
    assert!(!warning.contains("FAKE_CARGO_STDERR"));
    assert!(fixture
        .root()
        .join(".provenance/state/manifest.json")
        .exists());
}

#[test]
fn noisy_cargo_add_does_not_leak_before_failed_publication() {
    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);
    let before = fixture.cargo_manifest();
    let output = fixture
        .command()
        .args(["provenance", "init"])
        .env("FAKE_CARGO_ADD_NOISE", "1")
        .env("FAKE_CARGO_STALE_INIT_PLAN", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "{:?}", output.stdout);
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(!error.contains("FAKE_CARGO_STDERR"));
    assert!(!fixture
        .root()
        .join(".provenance/state/manifest.json")
        .exists());
    assert_eq!(fixture.cargo_manifest(), before);
}

#[test]
fn failed_cargo_add_includes_captured_diagnostics_and_rolls_back() {
    let fixture = CargoFixture::new(&[("app", "Cargo.toml")]);
    let before = fixture.cargo_manifest();
    let output = fixture
        .command()
        .args(["provenance", "init", "--quiet"])
        .env("FAKE_CARGO_ADD_NOISE", "1")
        .env("FAKE_CARGO_FAIL_AFTER_WRITE", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "{:?}", output.stdout);
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("FAKE_CARGO_STDERR"), "{error}");
    assert!(!fixture
        .root()
        .join(".provenance/state/manifest.json")
        .exists());
    assert_eq!(fixture.cargo_manifest(), before);
}
