//! Reads the help text of the spawned binary.

use std::process::Command;

/// The standard output of `provenance <arguments...>`, which must succeed.
pub fn help(arguments: &[&str]) -> String {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(arguments)
        .output()
        .expect("run provenance help");
    assert!(
        output.status.success(),
        "help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Windows help names the executable `provenance.exe`.
    String::from_utf8(output.stdout)
        .expect("help output is UTF-8")
        .replace("provenance.exe", "provenance")
}
