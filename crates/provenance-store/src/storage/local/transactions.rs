//! The transaction directories of the cache: which of their entries are
//! leftovers of a stopped write, and the removal of one transaction.
//!
//! A leftover is a real directory with a UTF-8 name. A write makes only such
//! directories, so a file, a link, a junction or a name that is not UTF-8 is
//! not transaction data: it stays, and the store does not follow a link.
//!
//! A removal has no directory sync. A power loss can bring a removed
//! transaction back, but no marker names it, so the next section entry
//! removes it again.

use super::recovery::contained_transaction;
use crate::layout::ProvenanceLayout;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;

/// The kind of write that owns a transaction directory of the cache.
#[derive(Clone, Copy, Debug)]
pub(crate) enum TransactionKind {
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
}

/// Removes each transaction of `kind` that no marker names, in name order.
/// `before_removal` gets each transaction before its removal.
///
/// The caller holds the section lock and no marker of `kind` exists: a write
/// holds the lock from the creation of its transaction to its marker, so no
/// live write owns such a transaction. The first failure stops the pass.
#[rule("rule_stopped_write_leaves_no_transaction_data")]
pub(crate) fn remove_leftovers(
    layout: &ProvenanceLayout,
    kind: TransactionKind,
    mut before_removal: impl FnMut(&Utf8Path) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    for transaction in leftovers(layout, kind)? {
        before_removal(&transaction)?;
        remove_transaction(&transaction)?;
    }
    Ok(())
}

/// The resolved path of each leftover of `kind`, in name order.
fn leftovers(
    layout: &ProvenanceLayout,
    kind: TransactionKind,
) -> anyhow::Result<Vec<Utf8PathBuf>> {
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
        found.push(contained_transaction(layout, &directory, &path)?);
    }
    found.sort();
    Ok(found)
}

/// Removes the transaction directory at `transaction` and its contents.
///
/// A failure gives the step, the path and the system error. When the system
/// error shows a cause that a person can correct, the text also tells the
/// correction. The contents that the call removed before the failure stay
/// removed.
pub(crate) fn remove_transaction(transaction: &Utf8Path) -> anyhow::Result<()> {
    std::fs::remove_dir_all(transaction).map_err(|error| {
        let step = correction(&error).map_or_else(
            || format!("remove transaction directory {transaction}"),
            |correction| format!("remove transaction directory {transaction} ({correction})"),
        );
        crate::refusal::caused(error, step)
    })
}

/// The correction for `error` when the system error shows its cause.
fn correction(error: &std::io::Error) -> Option<&'static str> {
    #[cfg(windows)]
    {
        const ERROR_SHARING_VIOLATION: i32 = 32;
        const ERROR_LOCK_VIOLATION: i32 = 33;
        if matches!(error.raw_os_error(), Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION)) {
            return Some("close the program that has a file of this directory open");
        }
    }
    #[cfg(unix)]
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        return Some(
            "give this process write permission to this directory and to the directory that \
             holds it",
        );
    }
    None
}
