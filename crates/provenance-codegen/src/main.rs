//! Write definitions or generate Rust types from an `OpenAPI` document.
use std::{env, fs, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("export") if args.len() == 3 => {
            let output = Path::new(&args[2]);
            fs::create_dir_all(output)?;
            let (openapi, mcp) = provenance_codegen::documents();
            for (name, value) in [
                ("openapi.json", openapi),
                ("mcp.json", mcp),
                ("fixtures.openapi.json", provenance_codegen::corpus()),
            ] {
                fs::write(
                    output.join(name),
                    format!("{}\n", serde_json::to_string_pretty(&value)?),
                )?;
            }
        }
        Some("rust") if args.len() == 4 => {
            let document = serde_json::from_slice(&fs::read(&args[2])?)?;
            fs::create_dir_all(&args[3])?;
            for (name, content) in provenance_codegen::rust_types(&document)? {
                let path = Path::new(&args[3]).join(name);
                fs::create_dir_all(path.parent().unwrap())?;
                fs::write(path, content)?;
            }
        }
        _ => return Err("usage: provenance-codegen export DIR | rust OPENAPI DIR".into()),
    }
    Ok(())
}
