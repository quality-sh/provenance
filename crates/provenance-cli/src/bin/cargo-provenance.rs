use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

// @provenance rule: rule_cargo_subcommand_uses_sibling_cli
fn main() {
    let mut arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|argument| argument == "provenance")
    {
        arguments.remove(0);
    }
    if arguments.len() == 1
        && arguments
            .first()
            .is_some_and(|argument| argument == "--version" || argument == "-V")
    {
        println!("cargo-provenance {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if arguments.first().is_none_or(|argument| argument != "init") {
        eprintln!("cargo provenance supports only `cargo provenance init`");
        std::process::exit(2);
    }
    arguments[0] = OsString::from("__cargo-init");

    let provenance = sibling_provenance().unwrap_or_else(|error| {
        eprintln!("matching sibling provenance executable is missing: {error}");
        std::process::exit(1);
    });
    let status = Command::new(provenance)
        .args(arguments)
        .status()
        .unwrap_or_else(|error| {
            eprintln!("failed to run the provenance executable: {error}");
            std::process::exit(1);
        });
    std::process::exit(status.code().unwrap_or(1));
}

fn sibling_provenance() -> std::io::Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let sibling = executable
        .parent()
        .ok_or_else(|| std::io::Error::other("cargo-provenance has no parent directory"))?
        .join(format!("provenance{}", std::env::consts::EXE_SUFFIX));
    if sibling.is_file() {
        Ok(sibling)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            sibling.display().to_string(),
        ))
    }
}
