use crate::layout::ProvenanceLayout;
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{ensure_supported_schema_version, SchemaVersion, SUPPORTED_SCHEMA_VERSION};
use provenance_macros::rule;
use serde::{de::DeserializeOwned, Serialize};
use std::io::Write;

pub mod guard;
pub(crate) mod journal;
mod read_only;
pub use guard::snapshot_state_under_guard;
pub use read_only::with_read_only_validation;

#[cfg(test)]
mod guard_tests;
#[cfg(test)]
mod journal_tests;

pub struct StateSnapshot {
    _directory: tempfile::TempDir,
    layout: ProvenanceLayout,
}

impl StateSnapshot {
    pub const fn layout(&self) -> &ProvenanceLayout {
        &self.layout
    }
}

pub fn with_repository_publication<R>(
    layout: &ProvenanceLayout,
    operation: impl FnOnce() -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    guard::synchronous_publication(layout, operation)
}

fn prepare_publication_lock(layout: &ProvenanceLayout) -> anyhow::Result<()> {
    let canonical_root = canonical_utf8(
        layout
            .provenance_dir()
            .parent()
            .unwrap_or_else(|| Utf8Path::new(".")),
        "repository path",
    )?;
    let provenance = layout.provenance_dir();
    create_real_directory(&provenance)?;
    let canonical_provenance = canonical_utf8(&provenance, "provenance path")?;
    anyhow::ensure!(
        canonical_provenance == canonical_root.join(".provenance"),
        "repository provenance directory is outside the repository"
    );

    let cache = layout.cache_dir();
    create_real_directory(&cache)?;
    let locks = cache.join("locks");
    create_real_directory(&locks)?;
    Ok(())
}

/// Resolves `path`, keeping the failure message that names what the path was.
fn canonical_utf8(path: &Utf8Path, description: &str) -> anyhow::Result<Utf8PathBuf> {
    let resolved = std::fs::canonicalize(path)
        .map_err(|error| anyhow::anyhow!("resolve {description} {path}: {error}"))?;
    Utf8PathBuf::from_path_buf(resolved)
        .map_err(|path| anyhow::anyhow!("{description} is not UTF-8: {}", path.display()))
}

/// Makes sure `path` is a real directory, creating it if it is absent.
///
/// This is the ancestor half of `rule_recovery_stays_in_cache`: called in turn
/// on `.provenance`, the cache and the lock directory, it settles that the
/// cache itself is not reached through a symlink. Everything below the cache is
/// settled by [`recovery_dir_inside_cache`]. That the two halves meet, so that
/// a symlinked cache is refused before recovery reads a marker, is checked by
/// `a_symlinked_cache_is_refused_before_recovery_runs`.
fn create_real_directory(path: &Utf8Path) -> anyhow::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Err(error) = std::fs::create_dir(path) {
                anyhow::ensure!(
                    error.kind() == std::io::ErrorKind::AlreadyExists,
                    "failed to create publication lock directory {path}: {error}"
                );
            }
        }
        Err(error) => return Err(error.into()),
    }
    let metadata = std::fs::symlink_metadata(path)?;
    anyhow::ensure!(
        metadata.file_type().is_dir() && !metadata.file_type().is_symlink(),
        "publication lock path contains a symlink component: {path}"
    );
    Ok(())
}

fn prepare_import_transactions_dir(layout: &ProvenanceLayout) -> anyhow::Result<()> {
    create_real_directory(&layout.import_transactions_dir())?;
    canonical_transactions_dir(layout).map(|_| ())
}

#[derive(Clone, Copy, Debug, serde::Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationPhase {
    Prepared,
    BackupCreated,
    Published,
}

#[derive(serde::Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PublicationMarker {
    schema_version: u32,
    transaction_dir: Utf8PathBuf,
    phase: PublicationPhase,
}

pub fn write_publication_marker(
    layout: &ProvenanceLayout,
    transaction_dir: &Utf8Path,
    phase: PublicationPhase,
) -> anyhow::Result<()> {
    let transaction_dir = validated_transaction_dir(layout, transaction_dir)?;
    let marker = PublicationMarker {
        schema_version: SUPPORTED_SCHEMA_VERSION.0,
        transaction_dir,
        phase,
    };
    let path = layout.publication_marker_path();
    std::fs::create_dir_all(layout.cache_dir())?;
    let mut temporary = tempfile::NamedTempFile::new_in(layout.cache_dir())?;
    temporary.write_all(&serde_json::to_vec(&marker)?)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    sync_directory(&layout.cache_dir())
}

pub fn clear_publication_marker(layout: &ProvenanceLayout) -> anyhow::Result<()> {
    let path = layout.publication_marker_path();
    if path.exists() {
        std::fs::remove_file(path)?;
        sync_directory(&layout.cache_dir())?;
    }
    Ok(())
}

pub fn recover_pending_publication(layout: &ProvenanceLayout) -> anyhow::Result<()> {
    let marker_path = layout.publication_marker_path();
    if !marker_path.exists() {
        return Ok(());
    }
    let marker: PublicationMarker = serde_json::from_str(&std::fs::read_to_string(&marker_path)?)?;
    ensure_supported_schema_version("publication marker", SchemaVersion(marker.schema_version))
        .with_context(|| {
            format!(
                "{marker_path}: publication marker has unsupported schema_version {}; expected {}",
                marker.schema_version, SUPPORTED_SCHEMA_VERSION.0
            )
        })?;
    if matches!(marker.phase, PublicationPhase::Published) && !marker.transaction_dir.exists() {
        validate_missing_transaction_dir(layout, &marker.transaction_dir)?;
        return clear_publication_marker(layout);
    }
    let transaction_dir = validated_transaction_dir(layout, &marker.transaction_dir)?;
    let backup = transaction_dir.join("backup-state");
    if !layout.state_dir().exists() {
        anyhow::ensure!(
            backup.exists(),
            "publication recovery found neither live state nor backup state"
        );
        std::fs::rename(&backup, layout.state_dir())?;
        sync_directory(&layout.provenance_dir())?;
    }
    if transaction_dir.exists() {
        std::fs::remove_dir_all(&transaction_dir)?;
    }
    clear_publication_marker(layout)
}

const OUTSIDE_REPOSITORY_CACHE: &str =
    "publication marker transaction is outside the repository cache";

const TRANSACTION_NOT_A_DIRECTORY: &str = "publication marker transaction is not a directory";

const TRANSACTION_HAS_SYMLINK: &str =
    "publication marker transaction path contains a symlink component";

/// What recovery is about to do with the directory it is checking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecoveryUse {
    /// Recovery will read, rename or delete the directory, so it has to be on
    /// disk and land where the name says it does.
    Touched,
    /// The directory is already gone and recovery only clears the marker, so
    /// nothing is left to resolve beyond the place it claimed to sit.
    AlreadyGone,
}

/// Publication recovery may only touch a directory named as a direct child
/// inside this repository's cache, and callers must operate on the returned
/// contained path, never the written one.
///
/// The path is judged as written: no relative component may be a symlink,
/// even when its target remains inside the cache. The cache root must
/// already be a real directory, established by [`create_real_directory`].
/// An [`RecoveryUse::AlreadyGone`] leaf may be absent, but a symlink
/// lingering at the leaf is still refused.
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
        .ok_or_else(|| anyhow::anyhow!("publication marker transaction has no parent"))?;
    let canonical_parent = std::fs::canonicalize(parent)?;
    anyhow::ensure!(
        canonical_parent == canonical_container.as_std_path(),
        OUTSIDE_REPOSITORY_CACHE
    );
    let name = candidate
        .file_name()
        .ok_or_else(|| anyhow::anyhow!(OUTSIDE_REPOSITORY_CACHE))?;
    let contained = canonical_container.join(name);
    if recovery_use == RecoveryUse::Touched {
        let resolved = canonical_utf8(candidate, "import transaction path")?;
        anyhow::ensure!(resolved == contained, OUTSIDE_REPOSITORY_CACHE);
        anyhow::ensure!(
            std::fs::symlink_metadata(&resolved)?.is_dir(),
            TRANSACTION_NOT_A_DIRECTORY
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
    // A marker may name the container through OS symlinks above it (macOS's
    // /var -> /private/var), where the candidate matches neither the written
    // nor the canonical container spelling. The ancestor that canonicalizes
    // to the container is that same protected directory under another
    // spelling; components below it are still judged as written.
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
                    std::fs::canonicalize(ancestor)
                        .is_ok_and(|resolved| resolved == canonical_container.as_std_path())
                })
                .and_then(|ancestor| {
                    candidate
                        .strip_prefix(ancestor)
                        .map(|relative| (ancestor, relative))
                        .ok()
                })
        })
        .ok_or_else(|| anyhow::anyhow!(OUTSIDE_REPOSITORY_CACHE))?;
    let mut written = written_container.to_path_buf();
    let component_count = relative.components().count();
    for (index, component) in relative.components().enumerate() {
        written.push(component.as_str());
        if recovery_use == RecoveryUse::AlreadyGone && index + 1 == component_count {
            match std::fs::symlink_metadata(&written) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
                Ok(metadata) => {
                    anyhow::ensure!(!metadata.file_type().is_symlink(), TRANSACTION_HAS_SYMLINK);
                    continue;
                }
            }
        }
        let metadata = std::fs::symlink_metadata(&written)?;
        anyhow::ensure!(!metadata.file_type().is_symlink(), TRANSACTION_HAS_SYMLINK);
    }
    Ok(())
}

fn validated_transaction_dir(
    layout: &ProvenanceLayout,
    transaction_dir: &Utf8Path,
) -> anyhow::Result<Utf8PathBuf> {
    let canonical_transactions = canonical_transactions_dir(layout)?;
    recovery_dir_inside_cache(
        &canonical_transactions,
        &layout.import_transactions_dir(),
        transaction_dir,
        RecoveryUse::Touched,
    )
}

fn validate_missing_transaction_dir(
    layout: &ProvenanceLayout,
    transaction_dir: &Utf8Path,
) -> anyhow::Result<()> {
    let canonical_transactions = canonical_transactions_dir(layout)?;
    recovery_dir_inside_cache(
        &canonical_transactions,
        &layout.import_transactions_dir(),
        transaction_dir,
        RecoveryUse::AlreadyGone,
    )
    .map(|_| ())
}

fn canonical_transactions_dir(layout: &ProvenanceLayout) -> anyhow::Result<Utf8PathBuf> {
    let canonical_cache = canonical_utf8(&layout.cache_dir(), "repository cache path")?;
    recovery_dir_inside_cache(
        &canonical_cache,
        &layout.cache_dir(),
        &layout.import_transactions_dir(),
        RecoveryUse::Touched,
    )
}

pub fn sync_directory(path: &Utf8Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    std::fs::File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

pub fn sync_tree(path: &Utf8Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(path)
        .map_err(|error| anyhow::anyhow!("list publication tree {path}: {error}"))?
    {
        let entry = entry?;
        let child = Utf8PathBuf::from_path_buf(entry.path())
            .map_err(|path| anyhow::anyhow!("publication path is not UTF-8: {}", path.display()))?;
        if entry.file_type()?.is_dir() {
            sync_tree(&child)?;
        } else {
            // Windows' FlushFileBuffers needs write access; a read-only
            // handle is refused with ERROR_ACCESS_DENIED. Unix fsyncs a
            // read descriptor happily.
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(cfg!(windows))
                .open(&child)
                .map_err(|error| anyhow::anyhow!("sync publication file {child}: {error}"))?;
            file.sync_all()
                .map_err(|error| anyhow::anyhow!("sync publication file {child}: {error}"))?;
        }
    }
    sync_directory(path)
}

pub fn snapshot_state(layout: &ProvenanceLayout) -> anyhow::Result<StateSnapshot> {
    with_repository_publication(layout, || guard::snapshot_body(layout))
}

impl crate::state_store::StateStore {
    pub fn with_repository_publication<R>(
        &self,
        operation: impl FnOnce() -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        with_repository_publication(&self.layout, operation)
    }

    pub(crate) fn mutate_jsonl_records<T, R>(
        &self,
        path: &Utf8Path,
        mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<R>,
    ) -> anyhow::Result<R>
    where
        T: DeserializeOwned + Serialize,
    {
        self.with_repository_publication(|| {
            let lock_path = self.layout.state_shard_lock_path(path)?;
            let result = crate::jsonl::mutate_jsonl_locked(path, &lock_path, mutate)?;
            self.record_shard_invalidation(path)?;
            Ok(result)
        })
    }

    /// Records one invalidation event for a mutated canonical shard,
    /// inside the publication section that committed the write. The
    /// journal is a work hint for catch-up; correctness never depends on
    /// it because the sweep hashes every family regardless.
    fn record_shard_invalidation(&self, path: &Utf8Path) -> anyhow::Result<()> {
        if !crate::operations::read_policy::journal_enabled(&self.layout) {
            return Ok(());
        }
        let Some((scope, family)) = journal::shard_event_key(&self.layout, path) else {
            return Ok(());
        };
        journal::record_events(
            &self.layout,
            &[journal::JournalEvent {
                sequence: 0,
                scope,
                family,
                record_id: String::new(),
                operation: "upsert".to_string(),
            }],
        )
    }
}

#[cfg(all(test, unix))]
mod containment_tests;
#[cfg(test)]
mod tests;
