//! Implementation aid: the build script chooses the bundled skill sources.
//! A workspace build reads only the root sources; a packaged crate reads its own copies.

#[path = "../build/skills.rs"]
mod skills;

use skills::NAMES;
use std::path::Path;

fn write_skills(root: &Path, skip: Option<&str>) {
    for name in NAMES.iter().filter(|name| Some(**name) != skip) {
        std::fs::create_dir_all(root.join(name)).unwrap();
        std::fs::write(root.join(name).join("SKILL.md"), format!("{name}\n")).unwrap();
    }
}

fn workspace() -> (tempfile::TempDir, std::path::PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let crate_root = temporary.path().join("crates/provenance-cli");
    std::fs::create_dir_all(&crate_root).unwrap();
    std::fs::write(temporary.path().join("Cargo.toml"), "[workspace]\n").unwrap();
    // Crate-local copies must not replace a missing root source.
    write_skills(&crate_root.join("skills"), None);
    (temporary, crate_root)
}

#[test]
fn workspace_build_fails_when_a_root_skill_source_is_missing() {
    let (temporary, crate_root) = workspace();
    write_skills(&temporary.path().join("skills"), Some("provenance-shaping"));
    let output = tempfile::tempdir().unwrap();
    let error = skills::generate(&crate_root, output.path()).unwrap_err();
    assert!(
        error.to_string().contains("provenance-shaping"),
        "unexpected error: {error}"
    );
}

#[test]
fn workspace_build_fails_when_the_root_skill_directory_is_missing() {
    let (_temporary, crate_root) = workspace();
    let output = tempfile::tempdir().unwrap();
    let error = skills::generate(&crate_root, output.path()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("workspace skill sources are missing"),
        "unexpected error: {error}"
    );
}

#[test]
fn packaged_crate_reads_its_own_skill_copies() {
    let (_temporary, crate_root) = workspace();
    std::fs::write(crate_root.join("Cargo.toml.orig"), "").unwrap();
    let output = tempfile::tempdir().unwrap();
    skills::generate(&crate_root, output.path()).unwrap();
    for name in NAMES {
        assert_eq!(
            std::fs::read_to_string(output.path().join("skills").join(name).join("SKILL.md"))
                .unwrap(),
            format!("{name}\n")
        );
    }
}
