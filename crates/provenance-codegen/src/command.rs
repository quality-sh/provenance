//! The command line of the `provenance-codegen` binary.
use std::{error::Error, fs, path::Path};

const USAGE: &str = "usage: provenance-codegen export DIR | rust OPENAPI DIR";

/// Runs one codegen command. `args` holds the command words after the
/// program name.
///
/// # Errors
///
/// Returns the usage text for an unknown command or a wrong argument count,
/// and returns any read, write, or generation failure.
pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    match args {
        [command, output] if command == "export" => export(Path::new(output)),
        [command, openapi, output] if command == "rust" => {
            rust(Path::new(openapi), Path::new(output))
        }
        _ => Err(USAGE.into()),
    }
}

/// Writes the operation documents, the compatibility table, and the fixture
/// corpus into `output`.
fn export(output: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(output)?;
    let (openapi, mcp) = crate::documents();
    for (name, value) in [
        ("openapi.json", openapi),
        ("mcp.json", mcp),
        (
            "compatibility.json",
            serde_json::to_value(provenance_core::protocol::host::COMPATIBILITY)?,
        ),
        ("fixtures.openapi.json", crate::corpus()),
    ] {
        fs::write(
            output.join(name),
            format!("{}\n", serde_json::to_string_pretty(&value)?),
        )?;
    }
    Ok(())
}

/// Generates Rust types from the `OpenAPI` document at `openapi` into
/// `output`.
fn rust(openapi: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let document = serde_json::from_slice(&fs::read(openapi)?)?;
    fs::create_dir_all(output)?;
    for (name, content) in crate::rust_types(&document)? {
        let path = output.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
    }
    Ok(())
}
