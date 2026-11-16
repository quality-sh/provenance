//! The per-user cache of a test in one isolated directory.
use std::path::Path;

/// Points every platform's per-user cache at one isolated directory.
pub fn use_user_cache(root: &Path) {
    let cache = root.join("cache");
    std::env::set_var("HOME", root);
    std::env::set_var("XDG_CACHE_HOME", &cache);
    std::env::set_var("LOCALAPPDATA", &cache);
}
