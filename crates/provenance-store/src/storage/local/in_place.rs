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
use crate::layout::ProvenanceLayout;
use crate::storage::{StateKey, WriteSet};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;

/// The name of the change list in a transaction.
const CHANGES: &str = "changes.json";
/// The name of the directory of the staged puts in a transaction.
const STAGED: &str = "staged";

/// The keys that one write changes: the change list of its transaction.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ChangeList {
    puts: Vec<String>,
    deletes: Vec<String>,
}

/// Writes each put of `writes` below the staged directory of `transaction`
/// and then the change list, and syncs them. When it returns, the
/// transaction holds all data that the install needs. It first refuses a set
/// whose live key is not a regular file, so that no install fails for a
/// cause that the write can see before its marker.
pub fn stage(
    live: &ProvenanceLayout,
    transaction: &Utf8Path,
    writes: &WriteSet,
) -> anyhow::Result<()> {
    ensure_installable(live, writes)?;
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
            .keys()
            .map(|key| key.as_str().to_owned())
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
/// each staged put that is still in the transaction replaces its live key,
/// and then each delete removes its live key. A staged put that is not in
/// the transaction is installed, because an install moves it. Thus the call
/// completes a write that an earlier call did in part.
///
/// A replace is a rename of one file onto one file, so a reader that has a
/// live file open with delete sharing, as each `std::fs` handle has, does
/// not make it fail.
#[rule("rule_write_passes_while_a_record_file_is_held")]
pub fn install(live: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()> {
    let changes = read_change_list(transaction)?;
    let state = live.state_dir();
    let staged = transaction.join(STAGED);
    for key in &changes.puts {
        if let Some(source) = staged_file(&staged, key)? {
            install_staged_file(&source, &state.join(key.as_str()))
                .map_err(|error| in_use(live, key, error))?;
            crate::test_probes::at("state_file_installed")?;
        }
    }
    for key in &changes.deletes {
        remove_synced_file(&state.join(key.as_str())).map_err(|error| in_use(live, key, error))?;
    }
    crate::test_probes::at("state_installed")
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

/// The error of an install or a removal of the live file of `key`. On
/// Windows, `ERROR_ACCESS_DENIED` (5) and `ERROR_SHARING_VIOLATION` (32) on
/// that step mean that a different process holds the file with no delete
/// sharing: the error is a [`StateFileInUse`]. Each other error stays as it
/// is.
#[rule("rule_held_record_file_refusal_names_the_file")]
fn in_use(live: &ProvenanceLayout, key: &str, error: anyhow::Error) -> anyhow::Error {
    let code = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<std::io::Error>())
        .and_then(std::io::Error::raw_os_error);
    match code {
        Some(code @ (5 | 32)) if cfg!(windows) => StateFileInUse {
            path: key
                .split('/')
                .fold(live.state_dir(), |path, part| path.join(part)),
            file: format!(".provenance/state/{key}"),
            source: std::io::Error::from_raw_os_error(code),
        }
        .into(),
        _ => error,
    }
}

/// Completes the write that a marker names after a stop: it runs the
/// install of `transaction` again. The caller holds the section lock and
/// then records the write as published.
#[rule("rule_stopped_write_completes_after_its_marker")]
pub fn roll_forward(live: &ProvenanceLayout, transaction: &Utf8Path) -> anyhow::Result<()> {
    install(live, transaction)
}

/// The change list of `transaction`. Each key must be a state key, so that
/// the install changes no file outside the state directory.
fn read_change_list(transaction: &Utf8Path) -> anyhow::Result<ChangeList> {
    let path = transaction.join(CHANGES);
    let bytes = std::fs::read(&path)
        .map_err(|error| crate::refusal::caused(error, format!("read {path}")))?;
    let changes: ChangeList = serde_json::from_slice(&bytes)
        .map_err(|error| crate::refusal::caused(error, format!("read {path}")))?;
    for key in changes.puts.iter().chain(&changes.deletes) {
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

/// Refuses `writes` when the live entry of a key that it changes is there and
/// is not a regular file: a rename or a removal of a file cannot replace it.
fn ensure_installable(live: &ProvenanceLayout, writes: &WriteSet) -> anyhow::Result<()> {
    let state = live.state_dir();
    for key in writes.puts().keys().chain(writes.deletes()) {
        let path = state.join(key.as_str());
        match std::fs::symlink_metadata(crate::long_path::extended(&path)?) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(crate::refusal::caused(error, format!("read {path}"))),
            Ok(metadata) => crate::write_error::ensure!(
                Fault,
                metadata.is_file(),
                "unsupported state entry: {path}"
            ),
        }
    }
    Ok(())
}

/// The staged file of `key` below `staged`, or `None` when the install
/// moved it. Each directory on the way is a real directory and the file is
/// a regular file, so that the install does not follow a link out of the
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
