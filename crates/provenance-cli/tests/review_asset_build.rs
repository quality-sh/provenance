#[path = "../build/review_assets.rs"]
mod review_assets;

#[test]
fn embeds_index_and_local_dependencies_in_a_stable_inventory() {
    let dir = tempfile::tempdir().unwrap();
    let assets = dir.path().join("assets");
    std::fs::create_dir_all(assets.join("assets")).unwrap();
    std::fs::write(assets.join("index.html"), "<!doctype html>").unwrap();
    std::fs::write(assets.join("assets/app.js"), "export const ready = true;").unwrap();
    let output = dir.path().join("bundle.rs");
    review_assets::generate(&assets, &output).unwrap();
    let first = std::fs::read_to_string(&output).unwrap();
    assert!(first.contains("\"/index.html\""));
    assert!(first.contains("\"/assets/app.js\""));
    assert!(first.contains("include_bytes!"));
    review_assets::generate(&assets, &output).unwrap();
    assert_eq!(first, std::fs::read_to_string(output).unwrap());
}

#[test]
fn rejects_missing_entry_and_reserved_paths() {
    let dir = tempfile::tempdir().unwrap();
    let assets = dir.path().join("assets");
    std::fs::create_dir(&assets).unwrap();
    let output = dir.path().join("bundle.rs");
    assert!(review_assets::generate(&assets, &output).is_err());
    std::fs::write(assets.join("index.html"), "<!doctype html>").unwrap();
    for name in ["metadata", "review-config", "v7", ".secret", "bad path"] {
        let path = assets.join(name);
        std::fs::write(&path, "not an asset").unwrap();
        assert!(review_assets::generate(&assets, &output).is_err(), "{name}");
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn rejects_file_and_directory_links() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let assets = dir.path().join("assets");
    std::fs::create_dir(&assets).unwrap();
    std::fs::write(assets.join("index.html"), "<!doctype html>").unwrap();
    let output = dir.path().join("bundle.rs");
    let secret = dir.path().join("secret");
    std::fs::write(&secret, "private").unwrap();
    symlink(&secret, assets.join("secret")).unwrap();
    assert!(review_assets::generate(&assets, &output).is_err());
    std::fs::remove_file(assets.join("secret")).unwrap();
    symlink(dir.path(), assets.join("outside")).unwrap();
    assert!(review_assets::generate(&assets, &output).is_err());
}

#[test]
fn rejects_operation_routes_with_any_version_segment() {
    for version in ["assets", "vNext", "v7beta", "operations"] {
        let dir = tempfile::tempdir().unwrap();
        let assets = dir.path().join("assets");
        std::fs::create_dir_all(assets.join(format!("{version}/operations"))).unwrap();
        std::fs::write(assets.join("index.html"), "<!doctype html>").unwrap();
        std::fs::write(assets.join(format!("{version}/operations/app.js")), "asset").unwrap();
        let output = dir.path().join("bundle.rs");
        assert!(
            review_assets::generate(&assets, &output).is_err(),
            "{version}"
        );
        assert!(!output.exists(), "no inventory is written after refusal");
    }
}

#[test]
fn accepts_paths_that_do_not_match_the_operation_route() {
    let dir = tempfile::tempdir().unwrap();
    let assets = dir.path().join("assets");
    for path in [
        "index.html",
        "assets/operations.js",
        "assets/operations/nested/app.js",
        "assets/maps/operations/app.js",
        "operations/assets/app.js",
        "assets/Operations/app.js",
        "vNext/app.js",
    ] {
        let file = assets.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, "asset").unwrap();
    }
    review_assets::generate(&assets, &dir.path().join("bundle.rs")).unwrap();
}
