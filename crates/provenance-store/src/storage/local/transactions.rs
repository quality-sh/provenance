//! The transaction directories of the cache and the marker of each kind of
//! whole-set write.
//!
//! A write makes its transaction directly in the transaction directory of
//! its kind, and later names the transaction in the marker of its kind. This
//! module owns the containment check of a transaction, the write and the
//! removal of a marker, the end of a write that a marker names, the removal
//! of a transaction that did not publish, and the error of each removal.
//!
//! A leftover is a real directory with a UTF-8 name. A write makes only such
//! directories, so a file, a link, a junction or a name that is not UTF-8 is
//! not transaction data: it stays, and the store does not follow a link.
//!
//! A removal has no directory sync. A power loss can bring a removed
//! transaction back after the store cleared its marker, and the next section
//! entry then removes it as a leftover.

use super::durability::{sync_directory, write_synced_file_named};
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
    /// The transaction directory of this kind in the cache.
    pub fn directory(self, layout: &ProvenanceLayout) -> Utf8PathBuf {
        match self {
            Self::Import => layout.import_transactions_dir(),
            Self::SourceEdit => layout.source_edit_transactions_dir(),
        }
    }

    /// The marker of this kind in the cache.
    pub fn marker(self, layout: &ProvenanceLayout) -> Utf8PathBuf {
        match self {
            Self::Import => layout.publication_marker_path(),
            Self::SourceEdit => layout.source_edit_marker_path(),
        }
    }
}

/// A step of the removal of the data of a write.
#[derive(Clone, Copy, Debug)]
pub enum RemovalStep {
    List,
    Read,
    Transaction,
    Unpublished,
    MarkerTemporary,
    PreparedFile,
    Backup,
}

/// The error of a step of the removal of the data of a write: the step, the
/// path that the step used, and the cause.
#[rule("rule_failed_removal_of_stopped_write_names_its_path")]
pub fn removal_failure<E>(step: RemovalStep, path: &Utf8Path, error: E) -> anyhow::Error
where
    E: std::error::Error + Send + Sync + 'static,
{
    let context = match step {
        RemovalStep::List => format!("list {path}"),
        RemovalStep::Read => format!("read {path}"),
        RemovalStep::Transaction => format!("remove transaction directory {path}"),
        RemovalStep::Unpublished => format!(
            "remove transaction directory {path} (no marker names this directory, thus the \
             store does not use its data, and a person can remove it)"
        ),
        RemovalStep::MarkerTemporary => format!("remove temporary file {path}"),
        RemovalStep::PreparedFile => format!("remove prepared file {path}"),
        RemovalStep::Backup => format!("remove backup {path}"),
    };
    crate::refusal::caused(error, context)
}

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
/// named its transaction: `remove` removes each such transaction, as the
/// error path of a live write does. The caller holds the section lock, and
/// no marker of `kind` exists, so no live write owns this data.
#[rule("rule_stopped_write_leaves_no_transaction_data")]
pub fn remove_leftovers(
    layout: &ProvenanceLayout,
    kind: TransactionKind,
    mut remove: impl FnMut(&Utf8Path) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    // Each transaction that no marker names, in name order, and then each
    // temporary file of the marker. The first failure stops the pass.
    for transaction in leftovers(layout, kind)? {
        remove(&transaction)?;
    }
    remove_marker_temporaries(layout, kind)
}

/// Removes the transaction directory of a write that did not publish. No
/// marker names it, so the store does not use its data.
pub fn remove_unpublished(transaction: &Utf8Path) -> anyhow::Result<()> {
    std::fs::remove_dir_all(transaction)
        .map_err(|error| removal_failure(RemovalStep::Unpublished, transaction, error))
}

/// The resolved path of each leftover of `kind`, in name order.
fn leftovers(layout: &ProvenanceLayout, kind: TransactionKind) -> anyhow::Result<Vec<Utf8PathBuf>> {
    let directory = kind.directory(layout);
    let listed = |error| removal_failure(RemovalStep::List, &directory, error);
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
    let listed = |error| removal_failure(RemovalStep::List, &cache, error);
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
        std::fs::remove_file(&path)
            .map_err(|error| removal_failure(RemovalStep::MarkerTemporary, &path, error))?;
    }
    Ok(())
}

/// Ends a write of `kind` whose state is final, published or rolled back:
/// removes the transaction that its marker names, if it exists, and then
/// the marker. It reads no file of the transaction, so a removal that
/// stopped or failed in part leaves a state that it can end. A live write
/// whose change is published succeeds when this call fails: its marker stays,
/// or a sync of the cache did not make the removal of the marker durable,
/// and the next section entry ends the write.
#[rule("rule_published_write_succeeds_when_its_cleanup_fails")]
pub fn end_marked_write(
    layout: &ProvenanceLayout,
    kind: TransactionKind,
    transaction: &Utf8Path,
) -> anyhow::Result<()> {
    if transaction.exists() {
        let contained = contained_transaction(layout, kind, transaction)?;
        std::fs::remove_dir_all(&contained)
            .map_err(|error| removal_failure(RemovalStep::Transaction, &contained, error))?;
    } else {
        contained_missing_transaction(layout, kind, transaction)?;
    }
    crate::test_probes::at("marked_transaction_removed")?;
    clear_marker(layout, kind)
}

/// Removes the marker of `kind`, if it exists, and then syncs the cache.
pub fn clear_marker(layout: &ProvenanceLayout, kind: TransactionKind) -> anyhow::Result<()> {
    match std::fs::remove_file(kind.marker(layout)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(()) => sync_directory(&layout.cache_dir()),
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
        "transaction",
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
        "transaction",
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
        "transaction directory",
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecoveryUse {
    Touched,
    AlreadyGone,
}

/// Publication recovery touches only the selected transaction directory inside the cache.
/// `subject` names the candidate in the error texts.
#[rule("rule_recovery_stays_in_cache")]
fn recovery_dir_inside_cache(
    canonical_container: &Utf8Path,
    written_container: &Utf8Path,
    candidate: &Utf8Path,
    recovery_use: RecoveryUse,
    subject: &str,
) -> anyhow::Result<Utf8PathBuf> {
    ensure_written_path_has_no_symlinks(
        canonical_container,
        written_container,
        candidate,
        recovery_use,
        subject,
    )?;
    let parent = candidate
        .parent()
        .ok_or_else(|| crate::refusal::fault!("{subject} {candidate} has no parent"))?;
    let canonical_parent = dunce::canonicalize(parent)?;
    crate::write_error::ensure!(
        Fault,
        canonical_parent == canonical_container.as_std_path(),
        "{subject} {candidate} is outside {canonical_container}"
    );
    let name = candidate.file_name().ok_or_else(|| {
        crate::refusal::fault!("{subject} {candidate} is outside {canonical_container}")
    })?;
    let contained = canonical_container.join(name);
    if recovery_use == RecoveryUse::Touched {
        let resolved = canonical_utf8(candidate, subject)?;
        crate::write_error::ensure!(
            Fault,
            resolved == contained,
            "{subject} {candidate} is outside {canonical_container}"
        );
        crate::write_error::ensure!(
            Fault,
            std::fs::symlink_metadata(&resolved)?.is_dir(),
            "{subject} {candidate} is not a directory"
        );
    }
    Ok(contained)
}

fn ensure_written_path_has_no_symlinks(
    canonical_container: &Utf8Path,
    written_container: &Utf8Path,
    candidate: &Utf8Path,
    recovery_use: RecoveryUse,
    subject: &str,
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
            crate::refusal::fault!("{subject} {candidate} is outside {canonical_container}")
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
                        "{subject} {candidate} contains a symlink component"
                    );
                    continue;
                }
            }
        }
        let metadata = std::fs::symlink_metadata(&written)?;
        crate::write_error::ensure!(
            Fault,
            !metadata.file_type().is_symlink(),
            "{subject} {candidate} contains a symlink component"
        );
    }
    Ok(())
}
