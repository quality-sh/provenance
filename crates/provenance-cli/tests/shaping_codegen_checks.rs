//! Implementation aid: run the planned generator checks in a prepared workspace.
use std::{path::Path, process::Command};

#[test]
fn operation_generator_checks_use_the_prepared_contracts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let mut tests: Vec<_> = std::fs::read_dir(root.join("tools/operation-codegen"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(".test.mjs")
        })
        .collect();
    tests.sort();
    assert!(!tests.is_empty());
    let output = Command::new("node")
        .arg("--test")
        .args(tests)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
