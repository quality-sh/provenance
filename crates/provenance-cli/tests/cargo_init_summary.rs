use std::fs;

#[path = "cargo_init/support.rs"]
mod support;
use support::CargoFixture;

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
        .env("PROVENANCE_TEST_STE100_ASSET_URL", "http://127.0.0.1:9/unavailable")
        .output()
        .unwrap();
    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));
    let first = String::from_utf8(first.stdout).unwrap();
    assert!(first.contains("  Cargo.toml (added the Provenance SDK dependency)\n"));
    assert!(first.contains("  Cargo.lock (created by Cargo)\n"));
    assert!(first.contains("  .provenance/state (manifest for scope \"default\")\n"));
    assert!(!first.contains("Dictionary:"));
    assert!(first.ends_with("Have your agent run provenance prime to get acclimated.\n"));
    let manifest = fs::read(fixture.root().join(".provenance/state/manifest.json")).unwrap();
    let cargo_manifest = fixture.cargo_manifest();
    let cargo_lock = fixture.cargo_lock();

    let second = fixture
        .command()
        .args(["provenance", "init"])
        .env("PROVENANCE_STE100_ASSET_DIR", &assets)
        .env("PROVENANCE_STE100_INDEX_DIR", &indexes)
        .env("PROVENANCE_TEST_STE100_ASSET_URL", "http://127.0.0.1:9/unavailable")
        .output()
        .unwrap();
    assert!(second.status.success(), "{}", String::from_utf8_lossy(&second.stderr));
    let second = String::from_utf8(second.stdout).unwrap();
    assert!(second.contains("No change.\n"));
    assert!(!second.contains("\nNew\n"));
    assert!(!second.contains("\nChanged\n"));
    assert!(second.ends_with("Have your agent run provenance prime to get acclimated.\n"));
    assert_eq!(fs::read(fixture.root().join(".provenance/state/manifest.json")).unwrap(), manifest);
    assert_eq!(fixture.cargo_manifest(), cargo_manifest);
    assert_eq!(fixture.cargo_lock(), cargo_lock);
}
