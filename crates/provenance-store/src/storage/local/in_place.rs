//! A whole set of many keys on local disk, installed file by file in the
//! live state directory.
//!
//! The write stages each put as a file in its transaction, with the change
//! list that names each put and each delete, and syncs them. The marker then
//! names the transaction. After the marker, the install replaces each live
//! put with its staged file and removes each delete. The install goes only
//! forward: the next section entry runs the same install for a write that
//! stopped after its marker. A replace keeps no other handle on the state
//! directory, so a reader that has a state file open with delete sharing
//! does not stop the write.

use super::durability::{
    install_staged_file, remove_synced_file, sync_tree, write_staged_file, write_synced_file,
};
use super::live_entry::{
    ensure_one_file_system, ensure_plain_state_tree, held_by_another_process, key_path, live_file,
};
use crate::layout::ProvenanceLayout;
use crate::storage::{StateKey, WriteSet};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;
use sha2::{Digest as _, Sha256};

/// The name of the change list in a transaction.
const CHANGES: &str = "changes.json";
/// The name of the directory of the staged puts in a transaction.
const STAGED: &str = "staged";

/// The keys that one write changes: the change list of its transaction.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ChangeList {
    puts: Vec<Put>,
    deletes: Vec<String>,
}

/// One put of a change list: the key and the SHA-256 digest of its bytes.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Put {
    key: String,
    sha256: [u8; 32],
}

/// Writes each put of `writes` below the staged directory of `transaction`
/// and then the change list, and syncs them. When it returns, the
/// transaction holds all data that the install needs. It first refuses a set
/// that an install cannot complete for a cause that the write can see before
/// its marker: a cache on a different file system, a link or a special entry
/// in the state directory, or a part of the path of a key that is not a
/// directory.
pub fn stage(
    live: &ProvenanceLayout,
    transaction: &Utf8Path,
    writes: &WriteSet,
) -> anyhow::Result<()> {
    ensure_installable(live, transaction, writes)?;
    let staged = transaction.join(STAGED);
    std::fs::create_dir(&staged)
        .map_err(|error| crate::refusal::caused(error, format!("make directory {staged}")))?;
    for (key, bytes) in writes.puts() {
        write_staged_file(&staged.join(key.as_str()), bytes)?;
    }
    sync_tree(&staged)?;
    let changes = ChangeList {
        puts: writes
            .puts()
            .iter()
            .map(|(key, bytes)| Put {
                key: key.as_str().to_owned(),
                sha256: Sha256::digest(bytes).into(),
            })
            .collect(),
        deletes: writes
            .deletes()
            .iter()
            .map(|key| key.as_str().to_owned())
            .collect(),
    };
    write_synced_file(&transaction.join(CHANGES), &serde_json::to_vec(&changes)?)
}

/// Installs the write of `transaction` on the state directory of `live`:
/// each staged put replaces its live key, and then each delete removes its
/// live key. A put whose staged file is gone is installed only when its live
/// file has the digest of the change list, because an install moves the
/// staged file. Thus the call completes a write that an earlier call did in
/// part, and refuses a transaction that lost a staged file.
///
/// The call checks each put and each delete before it changes a file, so a
/// refusal changes no file. A replace is a rename of one file onto one
/// file, so a reader that has a live file open with delete sharing, as each
/// `std::fs` handle has, does not make it fail. No step follows a link in
/// the path of a live key.
#[rule("rule_write_passes_while_a_record_file_is_held")]
pub fn install(live: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()> {
    let changes = read_change_list(transaction)?;
    let state = live.state_dir();
    let staged = transaction.join(STAGED);
    let mut replaces = Vec::new();
    for put in &changes.puts {
        let target = live_file(&state, &put.key)?;
        match staged_file(&staged, &put.key)? {
            Some(source) => {
                require_digest(&source, put, "staged file")?;
                replaces.push((put, source, target));
            }
            None => require_installed(&staged, &target, put)?,
        }
    }
    let removals = changes
        .deletes
        .iter()
        .map(|key| Ok((key, live_file(&state, key)?)))
        .collect::<anyhow::Result<Vec<_>>>()?;
    for (put, source, target) in replaces {
        install_staged_file(&source, &target)
            .map_err(|error| in_use(live, &target, &put.key, error))?;
        crate::test_probes::at("state_file_installed")?;
    }
    for (key, target) in removals {
        remove_synced_file(&target).map_err(|error| in_use(live, &target, key, error))?;
    }
    crate::test_probes::at("state_installed")
}

/// Refuses `put` when the file at `path` does not have its digest, with the
/// path of the file.
fn require_digest(path: &Utf8Path, put: &Put, what: &str) -> anyhow::Result<()> {
    let bytes = std::fs::read(crate::long_path::extended(path)?)
        .map_err(|error| crate::refusal::caused(error, format!("read {path}")))?;
    crate::write_error::ensure!(
        Fault,
        <[u8; 32]>::from(Sha256::digest(&bytes)) == put.sha256,
        "{what} {path} does not have the bytes that the change list names for {}",
        put.key
    );
    Ok(())
}

/// Refuses a put whose staged file is gone when its live file `target` does
/// not have the bytes of the put: the transaction lost the staged file
/// before its install.
fn require_installed(staged: &Utf8Path, target: &Utf8Path, put: &Put) -> anyhow::Result<()> {
    let missing = key_path(staged, &put.key);
    let installed = match std::fs::read(crate::long_path::extended(target)?) {
        Ok(bytes) => <[u8; 32]>::from(Sha256::digest(&bytes)) == put.sha256,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(crate::refusal::caused(error, format!("read {target}"))),
    };
    crate::write_error::ensure!(
        Fault,
        installed,
        "staged file {missing} is missing, and {target} does not have the bytes that the \
         change list names for {}",
        put.key
    );
    Ok(())
}

/// A live state file that a different process has open with no delete
/// sharing, so that the store cannot replace or remove it.
#[derive(Debug, thiserror::Error)]
#[error(
    "a different process has the state file {path} open and does not let other processes \
     replace it: close the file in that process, and then send the call again"
)]
pub struct StateFileInUse {
    /// The absolute path of the file.
    pub path: Utf8PathBuf,
    /// The path of the file relative to the repository, with `/` between
    /// its parts.
    pub file: String,
    #[source]
    pub source: std::io::Error,
}

/// The error of an install or a removal of the live file `target` of
/// `key`. When a different process holds the file with no delete sharing
/// (see [`held_by_another_process`]), the error is a [`StateFileInUse`].
/// Each other error stays as it is.
#[rule("rule_held_record_file_refusal_names_the_file")]
fn in_use(
    live: &ProvenanceLayout,
    target: &Utf8Path,
    key: &str,
    error: anyhow::Error,
) -> anyhow::Error {
    let code = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<std::io::Error>())
        .and_then(std::io::Error::raw_os_error);
    match code {
        Some(code) if held_by_another_process(target, code) => StateFileInUse {
            path: key_path(&live.state_dir(), key),
            file: format!(".provenance/state/{key}"),
            source: std::io::Error::from_raw_os_error(code),
        }
        .into(),
        _ => error,
    }
}

/// The change list of `transaction`. Each key must be a state key, so that
/// the install changes no file outside the state directory.
fn read_change_list(transaction: &Utf8Path) -> anyhow::Result<ChangeList> {
    let path = transaction.join(CHANGES);
    let bytes = std::fs::read(&path)
        .map_err(|error| crate::refusal::caused(error, format!("read {path}")))?;
    let changes: ChangeList = serde_json::from_slice(&bytes)
        .map_err(|error| crate::refusal::caused(error, format!("read {path}")))?;
    let puts = changes.puts.iter().map(|put| &put.key);
    for key in puts.chain(&changes.deletes) {
        // A `:` can start a drive prefix on Windows, and a join with a
        // prefix leaves the state directory.
        let valid = StateKey::new(key.as_str()).is_ok() && !key.contains(':');
        crate::write_error::ensure!(
            Fault,
            valid,
            "change list {path} names {key}, which is not a state key"
        );
    }
    Ok(changes)
}

/// Refuses `writes` when the install of a key that it changes cannot
/// complete: the transaction and the state directory are on two file
/// systems, the state directory holds a link or a special entry, a part of
/// the path of the key is not a directory, or the live entry of the key is
/// not a regular file.
fn ensure_installable(
    live: &ProvenanceLayout,
    transaction: &Utf8Path,
    writes: &WriteSet,
) -> anyhow::Result<()> {
    let state = live.state_dir();
    ensure_one_file_system(transaction, &state)?;
    ensure_plain_state_tree(&state)?;
    for key in writes.puts().keys().chain(writes.deletes()) {
        live_file(&state, key.as_str())?;
    }
    Ok(())
}

/// The staged file of `key` below `staged`, or `None` when it is not there.
/// Each directory on the way is a real directory and the file is a regular
/// file, so that the install does not follow a link out of the
/// transaction. A failure gives the path.
fn staged_file(staged: &Utf8Path, key: &str) -> anyhow::Result<Option<Utf8PathBuf>> {
    let mut path = staged.to_owned();
    let parts: Vec<&str> = key.split('/').collect();
    for (index, part) in std::iter::once("").chain(parts.iter().copied()).enumerate() {
        if !part.is_empty() {
            path.push(part);
        }
        let metadata = match std::fs::symlink_metadata(crate::long_path::extended(&path)?) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(crate::refusal::caused(error, format!("read {path}"))),
            Ok(metadata) => metadata,
        };
        let is_file = index == parts.len();
        let expected = if is_file {
            metadata.is_file()
        } else {
            metadata.is_dir()
        };
        crate::write_error::ensure!(
            Fault,
            expected && !metadata.file_type().is_symlink(),
            "staged entry {path} is not a real {}",
            if is_file { "file" } else { "directory" }
        );
    }
    Ok(Some(path))
}
