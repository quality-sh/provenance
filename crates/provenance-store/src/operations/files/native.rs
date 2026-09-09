//! Trusted native spelling is converted without resolving the selected file.
use super::{validate_relative, FileAccessRefusal as Refusal, Utf8Path, Utf8PathBuf};

pub(super) fn relative(root: &Utf8Path, path: &Utf8Path) -> Result<Utf8PathBuf, Refusal> {
    #[cfg(windows)]
    if path.is_absolute() {
        let relative = windows_relative(root.as_str(), path.as_str())?;
        validate_relative(&relative)?;
        return Ok(relative);
    }
    let path = if path.is_absolute() {
        path.strip_prefix(root).map_err(|_| Refusal::Denied)?
    } else {
        path
    };
    let mut parts = Vec::new();
    for part in path.components() {
        match part {
            camino::Utf8Component::Normal(name) => parts.push(name),
            camino::Utf8Component::CurDir => {}
            _ => return Err(Refusal::Denied),
        }
    }
    let relative: Utf8PathBuf = parts.join("/").into();
    validate_relative(&relative)?;
    Ok(relative)
}

#[cfg(any(windows, test))]
fn windows_relative(root: &str, path: &str) -> Result<Utf8PathBuf, Refusal> {
    let (root_prefix, root_parts) = windows_absolute(root)?;
    let (path_prefix, path_parts) = windows_absolute(path)?;
    if !root_prefix.eq_ignore_ascii_case(&path_prefix)
        || path_parts.len() < root_parts.len()
        || !root_parts
            .iter()
            .zip(&path_parts)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
    {
        return Err(Refusal::Denied);
    }
    Ok(path_parts[root_parts.len()..].join("/").into())
}

#[cfg(any(windows, test))]
fn windows_absolute(path: &str) -> Result<(String, Vec<String>), Refusal> {
    let normalized = path.replace('\\', "/");
    let spelling = normalized.strip_prefix("//?/").map_or_else(
        || normalized.clone(),
        |rest| {
            if rest
                .get(..4)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("UNC/"))
            {
                format!("//{}", &rest[4..])
            } else {
                rest.to_owned()
            }
        },
    );
    let (prefix, rest) = if let Some(unc) = spelling.strip_prefix("//") {
        let mut parts = unc.splitn(3, '/');
        let server = parts.next().ok_or(Refusal::Denied)?;
        let share = parts.next().ok_or(Refusal::Denied)?;
        if server.is_empty() || share.is_empty() || server.contains(':') || share.contains(':') {
            return Err(Refusal::Denied);
        }
        (
            format!("//{server}/{share}"),
            parts.next().unwrap_or_default(),
        )
    } else if spelling
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        && spelling.get(1..3) == Some(":/")
    {
        (spelling[..2].to_owned(), &spelling[3..])
    } else {
        return Err(Refusal::Denied);
    };
    let mut parts = Vec::new();
    for part in rest.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err(Refusal::Denied),
            name if name.contains(':') => return Err(Refusal::Denied),
            name => parts.push(name.to_owned()),
        }
    }
    Ok((prefix, parts))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_disk_and_unc_prefixes_preserve_relative_identity() {
        for (root, path, expected) in [
            (r"\\?\C:\Repo", r"c:\repo\src\Code.rs", "src/Code.rs"),
            (r"C:\Repo", r"\\?\c:\REPO\src\Code.rs", "src/Code.rs"),
            (
                r"\\?\UNC\server\share\Repo",
                r"\\SERVER\SHARE\repo\code.rs",
                "code.rs",
            ),
            (
                r"\\server\share\Repo",
                r"\\?\UNC\SERVER\share\repo\code.rs",
                "code.rs",
            ),
        ] {
            assert_eq!(windows_relative(root, path).unwrap(), expected);
        }
        for path in [
            r"D:\Repo\code.rs",
            r"C:\Other\code.rs",
            r"C:\Repo\..\code.rs",
            r"\\server\share\Repo\code.rs",
            r"C:\Repo\code.rs:stream",
        ] {
            assert!(windows_relative(r"C:\Repo", path).is_err(), "{path}");
        }
    }
    #[cfg(windows)]
    #[test]
    fn native_windows_relative_components_become_portable() {
        assert_eq!(
            relative(Utf8Path::new(r"\\?\C:\Repo"), Utf8Path::new(r"src\Code.rs")).unwrap(),
            "src/Code.rs"
        );
        assert_eq!(
            relative(
                Utf8Path::new(r"\\?\C:\Repo"),
                Utf8Path::new(r"C:\Repo\Code.rs")
            )
            .unwrap(),
            "Code.rs"
        );
    }
}
