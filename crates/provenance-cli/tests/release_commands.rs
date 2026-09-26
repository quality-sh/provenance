use std::{path::Path, process::Command};

#[test]
fn release_gate_checks_command_support() {
    let binary = assert_cmd::cargo::cargo_bin("provenance");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let output = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .arg(workspace.join(".github/scripts/verify-release-commands.py"))
        .arg(&binary)
        .current_dir(temporary.path())
        .output()
        .expect("run the release command gate");

    assert_eq!(
        output.status.success(),
        !cfg!(feature = "dogfood"),
        "release gate result does not match the command feature: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    if cfg!(feature = "dogfood") {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("Usage: provenance dogfood note"),
            "{stderr}"
        );
        assert!(stderr.contains("dogfood note was not rejected by the record parser"));
    }
}

#[test]
fn development_command_name_remains_reserved() {
    let error = provenance_core::ensure_record_id_assignable("dogfood").unwrap_err();
    assert_eq!(
        error.to_string(),
        "reserved record ID dogfood cannot be assigned"
    );
}
