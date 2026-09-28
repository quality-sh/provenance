use super::{canonical_utf8, sync_directory};
use crate::layout::ProvenanceLayout;
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{ensure_supported_schema_version, SchemaVersion, SUPPORTED_SCHEMA_VERSION};
use provenance_macros::rule;
use serde::Serialize;
use std::io::Write;

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

pub(super) fn validated_transaction_dir(
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

pub(super) fn validate_missing_transaction_dir(
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

pub(super) fn canonical_transactions_dir(
    layout: &ProvenanceLayout,
) -> anyhow::Result<Utf8PathBuf> {
    let canonical_cache = canonical_utf8(&layout.cache_dir(), "repository cache path")?;
    recovery_dir_inside_cache(
        &canonical_cache,
        &layout.cache_dir(),
        &layout.import_transactions_dir(),
        RecoveryUse::Touched,
    )
}

pub(super) fn create_real_directory(path: &Utf8Path) -> anyhow::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Err(error) = std::fs::create_dir(path) {
                if error.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(error).with_context(|| {
                        format!("failed to create publication lock directory {path}")
                    });
                }
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
