use std::{
    path::{Path, PathBuf},
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

/// A stand-in that prints `version` for `--version` and passes every other call
/// to `binary`, when `binary` is given.
fn stand_in(directory: &Path, version: &str, binary: Option<&Path>) -> PathBuf {
    let path = directory.join(if cfg!(windows) {
        "provenance.cmd"
    } else {
        "provenance"
    });
    let script = match (cfg!(windows), binary) {
        (true, Some(binary)) => format!(
            "@echo off\r\nif \"%~1\"==\"--version\" goto version\r\n\"{}\" %*\r\nexit /b %errorlevel%\r\n:version\r\necho {version}\r\n",
            binary.display()
        ),
        (true, None) => format!("@echo {version}\r\n"),
        (false, Some(binary)) => format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then echo '{version}'; exit 0; fi\nexec '{}' \"$@\"\n",
            binary.display()
        ),
        (false, None) => format!("#!/bin/sh\necho '{version}'\n"),
    };
    std::fs::write(&path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

/// Checks the gate result for a binary that names a clean build.
fn assert_command_support(binary: &Path) {
    let output = release_gate(binary);
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

/// The dogfood check runs for a clean and for a dirty local build. The gate
/// refuses a dirty build before that check, so the check then runs through a
/// stand-in that names a clean build and passes the other calls to the binary.
#[test]
fn release_gate_checks_command_support() {
    let binary = assert_cmd::cargo::cargo_bin("provenance");
    let version = Command::new(&binary).arg("--version").output().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let clean = stand_in(
        directory.path(),
        "provenance 0.4.0 (0123abcd)",
        Some(&binary),
    );
    assert_command_support(&clean);
    if String::from_utf8_lossy(&version.stdout).contains("-dirty") {
        let output = release_gate(&binary);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains("Release command gate failed: the version does not name a clean build"));
    } else {
        assert_command_support(&binary);
    }
}

/// Implementation aid: the release build job runs this gate before it publishes
/// anything, so a binary built from a changed checkout stops the release there.
#[test]
fn release_gate_refuses_a_dirty_build() {
    let directory = tempfile::tempdir().unwrap();
    let text = "provenance 0.4.0 (0123abcd-dirty)";
    let binary = stand_in(directory.path(), text, None);
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
