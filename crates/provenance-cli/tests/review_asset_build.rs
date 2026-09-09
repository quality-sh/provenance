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
