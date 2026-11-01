//! Locates the per-user cache directory of the operating system.

use std::path::PathBuf;

#[cfg(target_os = "windows")]
pub fn cache_directory() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

#[cfg(target_os = "macos")]
pub fn cache_directory() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Caches"))
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn cache_directory() -> Option<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
}
