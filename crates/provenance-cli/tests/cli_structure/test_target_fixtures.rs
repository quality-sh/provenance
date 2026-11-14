//! The test layout check on small fixture crates, one fault each.

use super::test_targets::{layout_faults, metadata};
use std::fs;

const ONE_TEST: &str = "#[test]\nfn passes() {}\n";

/// Writes a crate with `manifest` and `files`, and gives the faults of its layout.
fn faults(manifest: &str, files: &[(&str, &str)]) -> Vec<String> {
    let directory = tempfile::tempdir().expect("fixture directory");
    let root = directory.path();
    let manifest = format!(
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n{manifest}\n[workspace]\n"
    );
    fs::write(root.join("Cargo.toml"), manifest).expect("write fixture manifest");
    let library = ("src/lib.rs", "");
    for (path, text) in std::iter::once(&library).chain(files) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().expect("fixture file directory"))
            .expect("create fixture directory");
        fs::write(path, text).expect("write fixture file");
    }
    layout_faults(&metadata(root)).0
}

/// Gives the faults that start with `file` and contain `kind`.
fn naming<'a>(faults: &'a [String], file: &str, kind: &str) -> Vec<&'a String> {
    faults
        .iter()
        .filter(|fault| fault.starts_with(&format!("{file} ")) && fault.contains(kind))
        .collect()
}

#[test]
fn a_file_named_only_in_a_string_of_another_file_is_in_no_target() {
    let faults = faults(
        "autotests = false\n[[test]]\nname = \"answers\"\n",
        &[
            (
                "tests/answers.rs",
                "const FILE: &str = \"evidence.rs\";\n#[test]\nfn names() { assert!(!FILE.is_empty()); }\n",
            ),
            ("tests/evidence.rs", ONE_TEST),
        ],
    );
    assert_eq!(
        naming(&faults, "tests/evidence.rs", "in no test target").len(),
        1,
        "{faults:#?}"
    );
    assert_eq!(faults.len(), 1, "{faults:#?}");
}

#[test]
fn a_file_with_the_name_of_a_module_in_another_directory_is_in_no_target() {
    let faults = faults(
        "autotests = false\n[[test]]\nname = \"ideation_ids\"\n",
        &[
            (
                "tests/ideation_ids.rs",
                "#[path = \"ideation_ids/reads.rs\"]\nmod reads;\n",
            ),
            ("tests/ideation_ids/reads.rs", ONE_TEST),
            ("tests/reads.rs", ONE_TEST),
        ],
    );
    assert_eq!(
        naming(&faults, "tests/reads.rs", "in no test target").len(),
        1,
        "{faults:#?}"
    );
    assert_eq!(faults.len(), 1, "{faults:#?}");
}

#[test]
fn a_file_that_a_group_loads_and_that_is_its_own_target_is_in_two_targets() {
    let faults = faults(
        "autotests = false\n[[test]]\nname = \"group\"\npath = \"tests/group/main.rs\"\n\
         [[test]]\nname = \"reads\"\n",
        &[
            (
                "tests/group/main.rs",
                "#[path = \"../reads.rs\"]\nmod reads;\n",
            ),
            ("tests/reads.rs", ONE_TEST),
        ],
    );
    assert_eq!(
        naming(&faults, "tests/reads.rs", "loads 2 times").len(),
        1,
        "{faults:#?}"
    );
    assert_eq!(faults.len(), 1, "{faults:#?}");
}

#[test]
fn a_group_in_a_crate_with_automatic_targets_puts_its_files_in_two_targets() {
    let faults = faults(
        "",
        &[
            (
                "tests/group/main.rs",
                "#[path = \"../reads.rs\"]\nmod reads;\n",
            ),
            ("tests/reads.rs", ONE_TEST),
        ],
    );
    assert_eq!(
        naming(&faults, "tests/reads.rs", "loads 2 times").len(),
        1,
        "{faults:#?}"
    );
    assert_eq!(faults.len(), 1, "{faults:#?}");
}

#[test]
fn shared_support_with_tests_is_a_fault() {
    let faults = faults(
        "autotests = false\n[[test]]\nname = \"first\"\n[[test]]\nname = \"second\"\n",
        &[
            ("tests/first.rs", "mod writes_support;\n"),
            ("tests/second.rs", "mod writes_support;\n"),
            ("tests/writes_support.rs", ONE_TEST),
        ],
    );
    assert_eq!(
        naming(&faults, "tests/writes_support.rs", "declares tests").len(),
        1,
        "{faults:#?}"
    );
    assert_eq!(faults.len(), 1, "{faults:#?}");
}

#[test]
fn shared_support_without_tests_may_load_in_several_targets() {
    let faults = faults(
        "autotests = false\n[[test]]\nname = \"first\"\n[[test]]\nname = \"second\"\n",
        &[
            (
                "tests/first.rs",
                "mod writes_support;\nmod support {\n    pub mod files;\n}\n",
            ),
            (
                "tests/second.rs",
                "mod writes_support;\nmod support {\n    pub mod files;\n}\n",
            ),
            ("tests/writes_support.rs", "pub fn write() {}\n"),
            ("tests/support/files.rs", "pub fn file() {}\n"),
        ],
    );
    assert!(faults.is_empty(), "{faults:#?}");
}
