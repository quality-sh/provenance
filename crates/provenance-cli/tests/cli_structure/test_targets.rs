//! Finds the test files that are in no test target, or in more than one.

use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Reads `cargo metadata --no-deps` for the manifest in `directory`.
pub fn metadata(directory: &Path) -> Value {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(directory)
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed in {}: {}",
        directory.display(),
        String::from_utf8_lossy(&output.stderr),
    );
    serde_json::from_slice(&output.stdout).expect("parse cargo metadata")
}

/// Gives one fault for each top-level test file that is in no test target.
pub fn layout_faults(metadata: &Value) -> (Vec<String>, usize) {
    let root = PathBuf::from(metadata["workspace_root"].as_str().expect("workspace root"));
    let mut faults = Vec::new();
    let mut files_seen = 0usize;
    for package in metadata["packages"].as_array().expect("metadata packages") {
        let manifest = PathBuf::from(package["manifest_path"].as_str().expect("manifest path"));
        let tests = manifest.parent().expect("crate directory").join("tests");
        let Ok(entries) = fs::read_dir(&tests) else {
            continue;
        };
        let targets = package["targets"]
            .as_array()
            .expect("package targets")
            .iter()
            .filter(|target| target["kind"][0] == "test")
            .map(|target| PathBuf::from(target["src_path"].as_str().expect("target path")))
            .collect::<BTreeSet<_>>();
        let mut loaded = BTreeSet::new();
        let mut test_files = Vec::new();
        for entry in entries {
            let path = entry.expect("read tests entry").path();
            let group_main = path.join("main.rs");
            if group_main.is_file() {
                let text = fs::read_to_string(&group_main).expect("read group main.rs");
                for line in text.lines() {
                    if let Some(file) = line.trim().strip_prefix("#[path = \"../") {
                        loaded.insert(tests.join(file.trim_end_matches("\"]")));
                    }
                }
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                test_files.push((
                    path.clone(),
                    fs::read_to_string(&path).expect("read test file"),
                ));
            }
        }
        for (path, _) in &test_files {
            files_seen += 1;
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("file stem");
            let support = test_files.iter().any(|(other, text)| {
                other != path
                    && (text.contains(&format!("mod {stem};"))
                        || text.contains(&format!("\"{stem}.rs\"")))
            });
            if !(targets.contains(path) || loaded.contains(path) || support) {
                faults.push(format!(
                    "{} is in no test target of {}: add it to a group main.rs or as a [[test]]",
                    shown(path, &root),
                    package["name"],
                ));
            }
        }
    }
    (faults, files_seen)
}

/// Shows `path` from the workspace root, with `/` between components.
fn shown(path: &Path, root: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
