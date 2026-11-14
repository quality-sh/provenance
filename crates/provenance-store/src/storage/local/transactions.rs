//! The transaction directories of the cache and the marker of each kind of
//! whole-set write.
//!
//! A write makes its transaction directly in the transaction directory of
//! its kind, and later names the transaction in the marker of its kind. This
//! module owns the containment check of a transaction, its removal, the write
//! of a marker, and the removal of the data of a write that stopped before a
//! marker named its transaction.
//!
//! A leftover is a real directory with a UTF-8 name. A write makes only such
//! directories, so a file, a link, a junction or a name that is not UTF-8 is
//! not transaction data: it stays, and the store does not follow a link.
//!
//! A removal has no directory sync. A power loss can bring a removed
//! transaction back after the store cleared its marker, and the next section
//! entry then removes it as a leftover.

use super::durability::write_synced_file_named;
use super::section::canonical_utf8;
use crate::layout::ProvenanceLayout;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;

/// The kind of write that owns a transaction directory of the cache.
#[derive(Clone, Copy, Debug)]
pub enum TransactionKind {
    /// A whole-set write with no working-tree file.
    Import,
    /// A whole-set write with one working-tree file.
    SourceEdit,
}

impl TransactionKind {
    fn directory(self, layout: &ProvenanceLayout) -> Utf8PathBuf {
        match self {
            Self::Import => layout.import_transactions_dir(),
            Self::SourceEdit => layout.source_edit_transactions_dir(),
        }
    }

    fn marker(self, layout: &ProvenanceLayout) -> Utf8PathBuf {
        match self {
            Self::Import => layout.publication_marker_path(),
            Self::SourceEdit => layout.source_edit_marker_path(),
        }
    }
}

/// The next step for a failed removal of a transaction that no marker names.
const NO_MARKER_NAMES_IT: &str =
    "no marker names this directory, thus the store does not use its data, and a person can \
     remove it";

/// The suffix of the temporary file of a marker write.
const MARKER_TEMPORARY_SUFFIX: &str = ".tmp";

/// Writes `bytes` to the marker of `kind`, as `write_synced_file` does. The
/// temporary file of the write is `.<marker name>.<random>.tmp` in the
/// cache, so that the section entry can find it after a stop.
pub fn write_marker(
    layout: &ProvenanceLayout,
    kind: TransactionKind,
    bytes: &[u8],
) -> anyhow::Result<()> {
    let marker = kind.marker(layout);
    let prefix = marker_temporary_prefix(&marker);
    write_synced_file_named(
        &marker,
        bytes,
        tempfile::Builder::new()
            .prefix(&prefix)
            .suffix(MARKER_TEMPORARY_SUFFIX),
    )
}

fn marker_temporary_prefix(marker: &Utf8Path) -> String {
    format!(".{}.", marker.file_name().unwrap_or_default())
}

/// Removes the data of each write of `kind` that stopped before a marker
/// named its transaction. The caller holds the section lock, and no marker
/// of `kind` exists, so no live write owns this data.
#[rule("rule_stopped_write_leaves_no_transaction_data")]
pub fn remove_leftovers(
    layout: &ProvenanceLayout,
    kind: TransactionKind,
    mut before_removal: impl FnMut(&Utf8Path) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    // Each transaction that no marker names, in name order, and then each
    // temporary file of the marker. The first failure stops the pass.
    for transaction in leftovers(layout, kind)? {
        before_removal(&transaction)?;
        remove_directory(&transaction, || {
            format!("remove transaction directory {transaction} ({NO_MARKER_NAMES_IT})")
        })?;
    }
    remove_marker_temporaries(layout, kind)
}

/// The resolved path of each leftover of `kind`, in name order.
fn leftovers(layout: &ProvenanceLayout, kind: TransactionKind) -> anyhow::Result<Vec<Utf8PathBuf>> {
    let directory = kind.directory(layout);
    let listed = |error: std::io::Error| crate::refusal::caused(error, format!("list {directory}"));
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&directory).map_err(listed)? {
        let entry = entry.map_err(listed)?;
        if !entry.file_type().map_err(listed)?.is_dir() {
            continue;
        }
        let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
            continue;
        };
        found.push(contained_transaction(layout, kind, &path)?);
    }
    found.sort();
    Ok(found)
}

/// Removes each regular file in the cache that has the name of a temporary
/// file of the marker of `kind`.
fn remove_marker_temporaries(
    layout: &ProvenanceLayout,
    kind: TransactionKind,
) -> anyhow::Result<()> {
    let prefix = marker_temporary_prefix(&kind.marker(layout));
    let cache = layout.cache_dir();
    let listed = |error: std::io::Error| crate::refusal::caused(error, format!("list {cache}"));
    for entry in std::fs::read_dir(&cache).map_err(listed)? {
        let entry = entry.map_err(listed)?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if !name.starts_with(&prefix)
            || !name.ends_with(MARKER_TEMPORARY_SUFFIX)
            || !entry.file_type().map_err(listed)?.is_file()
        {
            continue;
        }
        let path = cache.join(name);
        std::fs::remove_file(&path).map_err(|error| {
            crate::refusal::caused(error, format!("remove temporary file {path}"))
        })?;
    }
    Ok(())
}

/// Removes the transaction directory at `transaction` and its contents. A
/// failure gives the step, the path and the system error. The contents that
/// the call removed before the failure stay removed.
pub fn remove_transaction(transaction: &Utf8Path) -> anyhow::Result<()> {
    remove_directory(transaction, || {
        format!("remove transaction directory {transaction}")
    })
}

fn remove_directory(path: &Utf8Path, step: impl FnOnce() -> String) -> anyhow::Result<()> {
    std::fs::remove_dir_all(path).map_err(|error| crate::refusal::caused(error, step()))
}

/// Removes the transaction of `kind` that a marker names, if it exists, after
/// the containment check. A transaction that does not exist is a success.
pub fn remove_marked_transaction(
    layout: &ProvenanceLayout,
    kind: TransactionKind,
    transaction: &Utf8Path,
) -> anyhow::Result<()> {
    if transaction.exists() {
        remove_transaction(&contained_transaction(layout, kind, transaction)?)
    } else {
        contained_missing_transaction(layout, kind, transaction)
    }
}

/// The resolved path of `transaction`, a real directory directly in the
/// transaction directory of `kind`, which is a real directory directly in
/// the cache.
pub fn contained_transaction(
    layout: &ProvenanceLayout,
    kind: TransactionKind,
    transaction: &Utf8Path,
) -> anyhow::Result<Utf8PathBuf> {
    let directory = kind.directory(layout);
    let canonical_directory = contained_directory(layout, kind)?;
    recovery_dir_inside_cache(
        &canonical_directory,
        &directory,
        transaction,
        RecoveryUse::Touched,
    )
}

/// Refuses a `transaction` that does not exist when its place is not
/// directly in the transaction directory of `kind`, or when a link is in
/// its place.
pub fn contained_missing_transaction(
    layout: &ProvenanceLayout,
    kind: TransactionKind,
    transaction: &Utf8Path,
) -> anyhow::Result<()> {
    let canonical_directory = contained_directory(layout, kind)?;
    recovery_dir_inside_cache(
        &canonical_directory,
        &kind.directory(layout),
        transaction,
        RecoveryUse::AlreadyGone,
    )
    .map(|_| ())
}

/// The resolved path of the transaction directory of `kind`, a real
/// directory directly in the cache.
pub fn contained_directory(
    layout: &ProvenanceLayout,
    kind: TransactionKind,
) -> anyhow::Result<Utf8PathBuf> {
    let canonical_cache = canonical_utf8(&layout.cache_dir(), "repository cache path")?;
    recovery_dir_inside_cache(
        &canonical_cache,
        &layout.cache_dir(),
        &kind.directory(layout),
        RecoveryUse::Touched,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecoveryUse {
    Touched,
    AlreadyGone,
}

/// Publication recovery touches only the selected transaction directory inside the cache.
#[rule("rule_recovery_stays_in_cache")]
fn recovery_dir_inside_cache(
    canonical_container: &Utf8Path,
    written_container: &Utf8Path,
    candidate: &Utf8Path,
    recovery_use: RecoveryUse,
) -> anyhow::Result<Utf8PathBuf> {
    ensure_written_path_has_no_symlinks(
        canonical_container,
        written_container,
        candidate,
        recovery_use,
    )?;
    let parent = candidate
        .parent()
        .ok_or_else(|| crate::refusal::fault!("transaction path {candidate} has no parent"))?;
    let canonical_parent = dunce::canonicalize(parent)?;
    crate::write_error::ensure!(
        Fault,
        canonical_parent == canonical_container.as_std_path(),
        "transaction path {candidate} is outside {canonical_container}"
    );
    let name = candidate.file_name().ok_or_else(|| {
        crate::refusal::fault!("transaction path {candidate} is outside {canonical_container}")
    })?;
    let contained = canonical_container.join(name);
    if recovery_use == RecoveryUse::Touched {
        let resolved = canonical_utf8(candidate, "transaction path")?;
        crate::write_error::ensure!(
            Fault,
            resolved == contained,
            "transaction path {candidate} is outside {canonical_container}"
        );
        crate::write_error::ensure!(
            Fault,
            std::fs::symlink_metadata(&resolved)?.is_dir(),
            "transaction path {candidate} is not a directory"
        );
    }
    Ok(contained)
}

fn ensure_written_path_has_no_symlinks(
    canonical_container: &Utf8Path,
    written_container: &Utf8Path,
    candidate: &Utf8Path,
    recovery_use: RecoveryUse,
) -> anyhow::Result<()> {
    let (written_container, relative) = candidate
        .strip_prefix(written_container)
        .map(|relative| (written_container, relative))
        .ok()
        .or_else(|| {
            candidate
                .strip_prefix(canonical_container)
                .map(|relative| (canonical_container, relative))
                .ok()
        })
        .or_else(|| {
            candidate
                .ancestors()
                .find(|ancestor| {
                    dunce::canonicalize(ancestor)
                        .is_ok_and(|resolved| resolved == canonical_container.as_std_path())
                })
                .and_then(|ancestor| {
                    candidate
                        .strip_prefix(ancestor)
                        .map(|relative| (ancestor, relative))
                        .ok()
                })
        })
        .ok_or_else(|| {
            crate::refusal::fault!("transaction path {candidate} is outside {canonical_container}")
        })?;
    let mut written = written_container.to_path_buf();
    let component_count = relative.components().count();
    for (index, component) in relative.components().enumerate() {
        written.push(component.as_str());
        if recovery_use == RecoveryUse::AlreadyGone && index + 1 == component_count {
            match std::fs::symlink_metadata(&written) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
                Ok(metadata) => {
                    crate::write_error::ensure!(
                        Fault,
                        !metadata.file_type().is_symlink(),
                        "transaction path {candidate} contains a symlink component"
                    );
                    continue;
                }
            }
        }
        let metadata = std::fs::symlink_metadata(&written)?;
        crate::write_error::ensure!(
            Fault,
            !metadata.file_type().is_symlink(),
            "transaction path {candidate} contains a symlink component"
        );
    }
    Ok(())
}
