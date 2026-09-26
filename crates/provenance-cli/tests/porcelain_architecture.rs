use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("CLI crate is in the workspace crates directory")
        .to_owned()
}

#[test]
fn workspace_has_an_independent_porcelain_crate() {
    let root = workspace_root();
    let workspace = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let manifest_path = root.join("crates/provenance-porcelain/Cargo.toml");

    assert!(
        workspace.contains("\"crates/provenance-porcelain\""),
        "the workspace must contain provenance-porcelain"
    );

    let manifest = std::fs::read_to_string(manifest_path).unwrap();
    for forbidden in ["provenance-transport", "clap", "rmcp"] {
        assert!(
            !manifest.lines().any(|line| {
                line.split_once('=')
                    .is_some_and(|(name, _)| name.trim() == forbidden)
            }),
            "provenance-porcelain must not depend on {forbidden}"
        );
    }
}

#[test]
fn human_facing_surfaces_depend_on_porcelain() {
    let root = workspace_root();
    for surface in ["provenance-cli", "provenance-transport"] {
        let manifest =
            std::fs::read_to_string(root.join("crates").join(surface).join("Cargo.toml")).unwrap();
        assert!(
            manifest.lines().any(|line| {
                line.split_once('=').is_some_and(|(name, _)| {
                    matches!(
                        name.trim(),
                        "provenance-porcelain" | "provenance-porcelain.workspace"
                    )
                })
            }),
            "{surface} must depend on provenance-porcelain"
        );
    }
}
