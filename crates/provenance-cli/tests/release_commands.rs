use std::{
    path::Path,
    process::{Command, Output},
};

fn release_gate(binary: &Path) -> Output {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let temporary = tempfile::tempdir().unwrap();
    Command::new(if cfg!(windows) { "python" } else { "python3" })
        .arg(workspace.join(".github/scripts/verify-release-commands.py"))
        .arg(binary)
        .current_dir(temporary.path())
        .output()
        .expect("run the release command gate")
}

#[test]
fn release_gate_checks_command_support() {
    let binary = assert_cmd::cargo::cargo_bin("provenance");
    let version = Command::new(&binary).arg("--version").output().unwrap();
    // A local build from a changed checkout is dirty; the gate refuses it first.
    if String::from_utf8_lossy(&version.stdout).contains("-dirty") {
        let output = release_gate(&binary);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains("Release command gate failed: the version does not name a clean build"));
        return;
    }
    let output = release_gate(&binary);

    assert_eq!(
        output.status.success(),
        !cfg!(feature = "dogfood"),
        "release gate result does not match the command feature: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    if cfg!(feature = "dogfood") {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Windows prints the executable name with its `.exe` suffix.
        assert!(
            stderr.contains("Usage: provenance dogfood note")
                || stderr.contains("Usage: provenance.exe dogfood note"),
            "{stderr}"
        );
        assert!(stderr.contains("dogfood note was not rejected by the record parser"));
    }
}

/// Implementation aid: the release build job runs this gate before it publishes
/// anything, so a binary built from a changed checkout stops the release there.
#[test]
fn release_gate_refuses_a_dirty_build() {
    let directory = tempfile::tempdir().unwrap();
    let text = "provenance 0.4.0 (0123abcd-dirty)";
    let binary = if cfg!(windows) {
        let binary = directory.path().join("provenance.cmd");
        std::fs::write(&binary, format!("@echo {text}\r\n")).unwrap();
        binary
    } else {
        let binary = directory.path().join("provenance");
        std::fs::write(&binary, format!("#!/bin/sh\necho '{text}'\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        binary
    };
    let output = release_gate(&binary);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains(text), "{stderr}");
    assert!(
        stderr.contains("Release command gate failed: the version does not name a clean build"),
        "{stderr}"
    );
}

#[test]
fn development_command_name_remains_reserved() {
    let error = provenance_core::ensure_record_id_assignable("dogfood").unwrap_err();
    assert_eq!(
        error.to_string(),
        "reserved record ID dogfood cannot be assigned"
    );
}
