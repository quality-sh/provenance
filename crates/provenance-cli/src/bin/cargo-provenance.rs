use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

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

    let status = Command::new(provenance_command())
        .args(arguments)
        .status()
        .unwrap_or_else(|error| {
            eprintln!("failed to run the provenance executable: {error}");
            std::process::exit(1);
        });
    std::process::exit(status.code().unwrap_or(1));
}

fn provenance_command() -> OsString {
    sibling_provenance().map_or_else(|| OsString::from("provenance"), PathBuf::into_os_string)
}

fn sibling_provenance() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let sibling = executable
        .parent()?
        .join(format!("provenance{}", std::env::consts::EXE_SUFFIX));
    sibling.is_file().then_some(sibling)
}
