//! The owner-only file in the per-user cache that holds the launch key of one review host
//! instance. The host writes it; the CLI and the notify client read it.

use anyhow::Context as _;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;

/// The owner-only file that holds the launch key of one review host instance.
pub fn path(nonce: &str) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        nonce.len() == 32 && nonce.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "the review host instance name is not valid"
    );
    let cache = provenance_store::user_cache::cache_directory()
        .context("cannot find the per-user cache directory")?;
    Ok(cache
        .join("provenance")
        .join("review-launch")
        .join(format!("{nonce}.key")))
}

/// Reads the key of a running host instance from the per-user cache.
pub fn read(nonce: &str) -> anyhow::Result<String> {
    let path = path(nonce)?;
    let metadata = std::fs::symlink_metadata(&path)
        .context("the review host has no launch key for this user")?;
    anyhow::ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "the review launch key is not a regular file"
    );
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o077 != 0 {
        anyhow::bail!("the review launch key is open to other users");
    }
    #[cfg(not(windows))]
    let value = std::fs::read_to_string(&path).context("cannot read the review launch key")?;
    #[cfg(windows)]
    let value = {
        let mut file = std::fs::File::open(&path).context("cannot read the review launch key")?;
        crate::owner_file::verify(&file)?;
        let mut value = String::new();
        std::io::Read::read_to_string(&mut file, &mut value)
            .context("cannot read the review launch key")?;
        value
    };
    anyhow::ensure!(is_secret(&value), "the review launch key is not valid");
    Ok(value)
}

/// Whether a value has the form of a launch key or a launch code.
pub fn is_secret(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
