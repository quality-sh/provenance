use std::io;
use std::path::{Path, PathBuf};

const NAMES: [&str; 4] = [
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

fn source_root(crate_root: &Path) -> io::Result<PathBuf> {
    if let Some(workspace) = crate_root.parent().and_then(Path::parent) {
        let manifest = workspace.join("Cargo.toml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        match std::fs::read_to_string(&manifest) {
            Ok(content)
                if content.contains("[workspace]")
                    && content
                        .contains("repository = \"https://github.com/quality-sh/provenance\"") =>
            {
                let source = workspace.join("skills");
                if !source.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("workspace skill sources are missing: {}", source.display()),
                    ));
                }
                return Ok(source);
            }
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            Ok(_) | Err(_) => {}
        }
    }
    Ok(crate_root.join("skills"))
}
