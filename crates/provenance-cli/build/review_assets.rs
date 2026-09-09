use std::{fmt::Write, fs, io, path::Path};

pub fn generate(root: &Path, output: &Path) -> io::Result<()> {
    let mut entries = Vec::new();
    collect(root, root, &mut entries)?;
    if !entries.iter().any(|(path, _)| path == "/index.html") {
        return Err(io::Error::other("review assets require index.html"));
    }
    entries.sort();
    let mut source = String::from("const ASSETS: &[(&str, &[u8])] = &[\n");
    for (url, file) in entries {
        writeln!(source, "({url:?}, include_bytes!({file:?})),").expect("write string");
    }
    source.push_str("];\n");
    fs::write(output, source)
}

fn collect(root: &Path, dir: &Path, entries: &mut Vec<(String, String)>) -> io::Result<()> {
    if fs::symlink_metadata(dir)?.file_type().is_symlink() {
        return Err(io::Error::other("review asset links are not allowed"));
    }
    println!("cargo:rerun-if-changed={}", dir.display());
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        let relative = path.strip_prefix(root).map_err(io::Error::other)?;
        let segments = relative
            .iter()
            .map(|segment| segment.to_str().unwrap_or_default())
            .collect::<Vec<_>>();
        let first = segments[0];
        if segments.iter().any(|segment| {
            segment.is_empty()
                || segment.starts_with('.')
                || !segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        }) || matches!(first, "metadata" | "review-config")
            || first.strip_prefix('v').is_some_and(|tail| {
                !tail.is_empty() && tail.bytes().all(|byte| byte.is_ascii_digit())
            })
            // The operation router matches any first segment, not only v<digits>.
            || (kind.is_file() && segments.len() == 3 && segments[1] == "operations")
        {
            return Err(io::Error::other("invalid or reserved review asset path"));
        }
        if kind.is_dir() {
            collect(root, &path, entries)?;
        } else if kind.is_file() {
            println!("cargo:rerun-if-changed={}", path.display());
            let absolute = path.canonicalize()?;
            entries.push((
                format!("/{}", segments.join("/")),
                absolute
                    .to_str()
                    .ok_or_else(|| io::Error::other("review asset path must be UTF-8"))?
                    .to_owned(),
            ));
        } else {
            return Err(io::Error::other("review assets must be regular files"));
        }
    }
    Ok(())
}
