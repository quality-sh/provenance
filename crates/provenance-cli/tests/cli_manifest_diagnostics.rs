use assert_cmd::Command;
use provenance_core::protocol::read_failure::STATE_CORRECTIONS;
use provenance_macros::verifies;
use std::path::{Path, PathBuf};

/// A repository directory below a different working directory, whose manifest
/// does not parse at line 1.
fn malformed_manifest(temporary: &Path) -> (PathBuf, PathBuf) {
    let selected = temporary.join("selected");
    let manifest = selected
        .join(".provenance")
        .join("state")
        .join("manifest.json");
    std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    std::fs::write(&manifest, MALFORMED).unwrap();
    (selected, manifest)
}

const MALFORMED: &[u8] = b"{bad";

/// The command refuses with the line of the manifest of the selected
/// repository and changes no file of it.
fn assert_refused(output: &std::process::Output, selected: &Path, manifest: &Path) {
    let stderr = String::from_utf8(output.stderr.clone()).unwrap();
    let expected =
        format!("Error: manifest.json line 1: key must be a string; {STATE_CORRECTIONS}\n");
    assert_eq!((output.status.code(), stderr), (Some(1), expected));
    assert_eq!(output.stdout, [] as [u8; 0]);
    assert_eq!(std::fs::read(manifest).unwrap(), MALFORMED);
    assert!(!selected.join(".provenance/cache/provenance.db").exists());
    assert!(!selected.join(".provenance/state/scopes").exists());
}

/// A list read with `--repo` from a different working directory names the line
/// of the manifest of the selected repository, the correction of that line
/// and then the two procedures.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn requirements_list_names_the_line_of_the_selected_malformed_manifest() {
    let temporary = tempfile::tempdir().unwrap();
    let (selected, manifest) = malformed_manifest(temporary.path());

    let output = Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(temporary.path())
        .args(["api", "requirements", "--repo", selected.to_str().unwrap()])
        .output()
        .unwrap();

    assert_refused(&output, &selected, &manifest);
}

/// An init with `--path` from a different working directory names the line of
/// the manifest of the selected repository, the correction of that line and
/// then the two procedures.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn init_names_the_line_of_the_selected_malformed_manifest() {
    let temporary = tempfile::tempdir().unwrap();
    let (selected, manifest) = malformed_manifest(temporary.path());

    let output = Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(temporary.path())
        .args(["init", "--path", selected.to_str().unwrap()])
        .output()
        .unwrap();

    assert_refused(&output, &selected, &manifest);
}
