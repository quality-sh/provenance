//! The key that signs the read cursors of the file adapter: 32 bytes in
//! `read-cursor.key` of the cache.

use crate::jsonl::AdvisoryLock;
use crate::layout::ProvenanceLayout;
use crate::storage::local::durability::write_synced_file;
use anyhow::Context;
use camino::Utf8Path;
use provenance_macros::rule;
use std::io::Read;

/// The number of bytes of a valid key.
const KEY_BYTES: usize = 32;

/// A cache with no cursor key or a damaged cursor key does not stop a paged
/// read: the read makes one new key, and callers that find no key at the
/// same time sign with that one key.
///
/// A valid key is read without the lock, so a cache that this process
/// cannot write still gives its key. The wait for the lock blocks the
/// calling thread, and a holder keeps the lock only for one synced write.
#[rule("rule_read_replaces_an_invalid_cursor_key")]
pub(super) fn cursor_key(layout: &ProvenanceLayout) -> anyhow::Result<[u8; KEY_BYTES]> {
    let path = layout.cursor_key_path();
    if let Some(key) = stored_key(&path)? {
        return Ok(key);
    }
    replace_key(layout, &path).with_context(|| {
        format!(
            "the cursor key file {path} is absent or does not hold a key, and the store \
             cannot replace it"
        )
    })
}

/// The key that the holder of the lock finds or writes at `path`.
fn replace_key(layout: &ProvenanceLayout, path: &Utf8Path) -> anyhow::Result<[u8; KEY_BYTES]> {
    let _lock = AdvisoryLock::acquire(&layout.cursor_key_lock_path())?;
    if let Some(key) = stored_key(path)? {
        return Ok(key);
    }
    let mut key = [0; KEY_BYTES];
    key[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    key[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    write_synced_file(path, &key)?;
    Ok(key)
}

/// The key in the file at `path`, or `None` when the file does not exist
/// or does not hold 32 bytes. A failure to open or read the file gives
/// `read <path>` and the I/O error.
fn stored_key(path: &Utf8Path) -> anyhow::Result<Option<[u8; KEY_BYTES]>> {
    let read_failure =
        |error: std::io::Error| crate::refusal::caused(error, format!("read {path}"));
    let file = match std::fs::File::open(crate::long_path::extended(path)?) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(read_failure(error)),
    };
    let mut bytes = Vec::with_capacity(KEY_BYTES + 1);
    file.take(u64::try_from(KEY_BYTES)? + 1)
        .read_to_end(&mut bytes)
        .map_err(read_failure)?;
    Ok(bytes.try_into().ok())
}
