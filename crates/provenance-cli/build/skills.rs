use std::io;
use std::path::{Path, PathBuf};

pub const NAMES: [&str; 4] = [
    "provenance-fork-tournament",
    "provenance-grounded-writing",
    "provenance-shaping",
    "provenance-swarm-backtrace",
];

pub fn generate(crate_root: &Path, output: &Path) -> io::Result<()> {
    let source = source_root(crate_root)?;
    let sources = NAMES
        .iter()
        .map(|name| {
            let file = source.join(name).join("SKILL.md");
            println!("cargo:rerun-if-changed={}", file.display());
            std::fs::read(&file).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("cannot read {}: {error}", file.display()),
                )
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    for (name, content) in NAMES.iter().zip(sources) {
        let directory = output.join("skills").join(name);
        std::fs::create_dir_all(&directory)?;
        std::fs::write(directory.join("SKILL.md"), content)?;
    }
    Ok(())
}

/// Cargo writes `Cargo.toml.orig` into every package archive. Only a packaged
/// crate reads the crate-local skill copies. A workspace build reads the root
/// sources and fails when they are missing.
fn source_root(crate_root: &Path) -> io::Result<PathBuf> {
    if crate_root.join("Cargo.toml.orig").is_file() {
        return Ok(crate_root.join("skills"));
    }
    let source = crate_root.join("../../skills");
    if !source.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("workspace skill sources are missing: {}", source.display()),
        ));
    }
    Ok(source)
}
