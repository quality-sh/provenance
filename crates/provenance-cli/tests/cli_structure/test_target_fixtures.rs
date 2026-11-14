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

#[test]
fn a_group_main_that_is_not_a_test_target_is_a_fault() {
    let faults = faults("autotests = false\n", &[("tests/group/main.rs", ONE_TEST)]);
    assert_eq!(
        naming(&faults, "tests/group/main.rs", "not a test target").len(),
        1,
        "{faults:#?}"
    );
    assert_eq!(faults.len(), 1, "{faults:#?}");
}

#[test]
fn a_file_with_tests_in_a_subdirectory_that_loads_nowhere_is_a_fault() {
    let faults = faults(
        "autotests = false\n[[test]]\nname = \"group\"\npath = \"tests/group/main.rs\"\n",
        &[
            ("tests/group/main.rs", "mod reads;\n"),
            ("tests/group/reads.rs", ONE_TEST),
            ("tests/group/writes.rs", ONE_TEST),
            ("tests/group/data/input.rs", "fn main() {}\n"),
            ("tests/group/data/broken.rs", "fn main( {\n"),
            ("tests/group/data/notes.txt", "#[test]\n"),
        ],
    );
    assert_eq!(
        naming(&faults, "tests/group/writes.rs", "in no test target").len(),
        1,
        "{faults:#?}"
    );
    assert_eq!(faults.len(), 1, "{faults:#?}");
}

const TWO_TARGETS: &str =
    "autotests = false\n[[test]]\nname = \"first\"\n[[test]]\nname = \"second\"\n";

#[test]
fn a_module_of_a_file_loads_from_the_directory_with_the_file_name() {
    let faults = faults(
        TWO_TARGETS,
        &[
            ("tests/first.rs", "mod a;\n"),
            ("tests/a.rs", "mod reads;\n"),
            ("tests/a/reads.rs", ONE_TEST),
            ("tests/second.rs", "#[path = \"a/reads.rs\"]\nmod reads;\n"),
        ],
    );
    assert_eq!(
        naming(&faults, "tests/a/reads.rs", "loads 2 times").len(),
        1,
        "{faults:#?}"
    );
    assert_eq!(faults.len(), 1, "{faults:#?}");
}

#[test]
fn a_module_of_a_mod_rs_file_loads_from_the_directory_of_that_file() {
    let faults = faults(
        TWO_TARGETS,
        &[
            ("tests/first.rs", "mod a;\n"),
            ("tests/a/mod.rs", "mod y;\n"),
            ("tests/a/y.rs", ONE_TEST),
            ("tests/second.rs", "#[path = \"a/y.rs\"]\nmod y;\n"),
        ],
    );
    assert_eq!(
        naming(&faults, "tests/a/y.rs", "loads 2 times").len(),
        1,
        "{faults:#?}"
    );
    assert_eq!(faults.len(), 1, "{faults:#?}");
}

#[test]
fn a_path_in_an_inline_module_starts_at_the_directory_of_that_module() {
    let faults = faults(
        TWO_TARGETS,
        &[
            ("tests/first.rs", "mod a;\n"),
            (
                "tests/a.rs",
                "mod inner {\n    #[path = \"z.rs\"]\n    mod z;\n}\n",
            ),
            ("tests/a/inner/z.rs", ONE_TEST),
            ("tests/second.rs", "#[path = \"a/inner/z.rs\"]\nmod z;\n"),
        ],
    );
    assert_eq!(
        naming(&faults, "tests/a/inner/z.rs", "loads 2 times").len(),
        1,
        "{faults:#?}"
    );
    assert_eq!(faults.len(), 1, "{faults:#?}");
}

#[test]
fn two_cfg_alternatives_of_one_module_load_its_file_once() {
    let faults = faults(
        "autotests = false\n[[test]]\nname = \"first\"\n",
        &[
            ("tests/first.rs", "mod a;\n"),
            (
                "tests/a.rs",
                "#[cfg(unix)]\nmod x;\n#[cfg(not(unix))]\nmod x;\n",
            ),
            ("tests/a/x.rs", ONE_TEST),
        ],
    );
    assert!(faults.is_empty(), "{faults:#?}");
}
