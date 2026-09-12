use crate::{canonical_digest, layout::ProvenanceLayout, state_store::StateStore};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::review::{
    CycleEntry, JournalEntry, RequirementSnapshot, ReviewEntry, SnapshotRef, REVIEW_SCHEMA_VERSION,
};
use provenance_core::{Requirement, ScopeId, StableId};
use serde::{de::DeserializeOwned, Serialize};
use std::io::{Read, Write};

pub(super) const ENTRY_BYTES: u64 = 65_536;

pub(super) fn directory(layout: &ProvenanceLayout, scope: &ScopeId) -> Utf8PathBuf {
    layout.scopes_dir().join(scope.as_str()).join("review")
}

pub(super) fn entry_path(
    layout: &ProvenanceLayout,
    scope: &ScopeId,
    request: &StableId,
) -> Utf8PathBuf {
    directory(layout, scope).join("journal").join(format!(
        "{}.json",
        canonical_digest::sha256(request.as_str().as_bytes())
    ))
}

pub(super) fn snapshot_path(
    layout: &ProvenanceLayout,
    scope: &ScopeId,
    id: &StableId,
) -> Utf8PathBuf {
    directory(layout, scope)
        .join("snapshots")
        .join(format!("{}.json", id.as_str()))
}

pub(super) fn read_entry(
    layout: &ProvenanceLayout,
    path: &Utf8Path,
) -> anyhow::Result<ReviewEntry> {
    let JournalEntry::Requirement(entry) = read_journal_entry(layout, path)? else {
        anyhow::bail!("request ID belongs to a Discussion or decision-cycle write");
    };
    anyhow::ensure!(
        entry.schema_version == REVIEW_SCHEMA_VERSION,
        "unsupported review journal version"
    );
    Ok(*entry)
}

pub(super) fn read_journal_entry(
    layout: &ProvenanceLayout,
    path: &Utf8Path,
) -> anyhow::Result<JournalEntry> {
    let entry: JournalEntry = read_bounded(layout, path, ENTRY_BYTES)?;
    let version = match &entry {
        JournalEntry::Requirement(e) => e.schema_version,
        JournalEntry::Discussion(e) => e.schema_version,
        JournalEntry::Cycle(e) => e.schema_version,
    };
    anyhow::ensure!(
        version == REVIEW_SCHEMA_VERSION,
        "unsupported review journal version"
    );
    Ok(entry)
}

pub(super) fn read_bounded<T: DeserializeOwned>(
    layout: &ProvenanceLayout,
    path: &Utf8Path,
    limit: u64,
) -> anyhow::Result<T> {
    let mut file = regular_file(layout, path)?;
    anyhow::ensure!(
        file.metadata()?.len() <= limit,
        "review entry exceeds the byte budget"
    );
    let mut bytes = Vec::new();
    (&mut file).take(limit + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() as u64 <= limit,
        "review entry exceeds the byte budget"
    );
    Ok(serde_json::from_slice(&bytes)?)
}

pub(super) fn regular_file(
    layout: &ProvenanceLayout,
    path: &Utf8Path,
) -> anyhow::Result<std::fs::File> {
    use crate::operations::files::{native_relative, RepositoryFiles};

    // Resolve only the trusted repository root. Evidence components stay lexical
    // and open relative to held directories, with no symlink or reparse traversal.
    let relative = path.strip_prefix(layout.root())?;
    let root = layout.root().canonicalize_utf8()?;
    let relative = native_relative(&root, relative)?;
    Ok(RepositoryFiles::open(&root)?.open_file(&relative)?.file)
}

pub(super) fn write_new<T: Serialize>(path: &Utf8Path, value: &T) -> anyhow::Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(&canonical_digest::canonical_bytes(value)?)?;
    Ok(())
}

pub(super) fn snapshot(
    layout: &ProvenanceLayout,
    record: &Requirement,
) -> anyhow::Result<SnapshotRef> {
    let id = new_id();
    let value = RequirementSnapshot {
        schema_version: REVIEW_SCHEMA_VERSION,
        record: record.clone(),
    };
    let bytes = canonical_digest::canonical_bytes(&value)?;
    write_new(&snapshot_path(layout, &record.scope_id, &id), &value)?;
    Ok(SnapshotRef {
        id,
        digest: canonical_digest::digest(&bytes),
        bytes: bytes.len() as u64,
        fields: super::snapshot::fields(record)?,
    })
}

pub(super) fn new_id() -> StableId {
    StableId::new(uuid::Uuid::new_v4().to_string()).expect("UUID uses valid stable ID characters")
}

pub(super) fn record_digest(record: &Requirement) -> anyhow::Result<String> {
    let value = RequirementSnapshot {
        schema_version: REVIEW_SCHEMA_VERSION,
        record: record.clone(),
    };
    Ok(canonical_digest::digest(
        &canonical_digest::canonical_bytes(&value)?,
    ))
}

pub(super) fn etag(record: &Requirement, occurrence: Option<&StableId>) -> anyhow::Result<String> {
    Ok(canonical_digest::digest(
        &canonical_digest::canonical_bytes(&(record, occurrence))?,
    ))
}

impl StateStore {
    pub(crate) fn review_entries(&self, scope: &ScopeId) -> anyhow::Result<Vec<ReviewEntry>> {
        Ok(self
            .journal_entries(scope)?
            .into_iter()
            .filter_map(|e| match e {
                JournalEntry::Requirement(e) => Some(*e),
                _ => None,
            })
            .collect())
    }

    pub(super) fn cycle_entries(&self, scope: &ScopeId) -> anyhow::Result<Vec<CycleEntry>> {
        Ok(self
            .journal_entries(scope)?
            .into_iter()
            .filter_map(|e| match e {
                JournalEntry::Cycle(e) => Some(*e),
                _ => None,
            })
            .collect())
    }

    pub(super) fn journal_entries(&self, scope: &ScopeId) -> anyhow::Result<Vec<JournalEntry>> {
        let dir = directory(&self.layout, scope).join("journal");
        if !dir.try_exists()? {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        for file in std::fs::read_dir(dir)? {
            let file = file?;
            let path = Utf8PathBuf::from_path_buf(file.path())
                .map_err(|_| anyhow::anyhow!("non-UTF-8 review path"))?;
            let entry = read_journal_entry(&self.layout, &path)?;
            anyhow::ensure!(
                *entry.scope_id() == *scope
                    && path == entry_path(&self.layout, scope, entry.request_id()),
                "review entry address mismatch"
            );
            entries.push(entry);
        }
        Ok(entries)
    }

    pub(super) fn head(&self, record: &Requirement) -> anyhow::Result<Option<ReviewEntry>> {
        let entries = self
            .review_entries(&record.scope_id)?
            .into_iter()
            .filter(|entry| entry.requirement_id == record.id)
            .collect::<Vec<_>>();
        let head = validated_head(&entries)?;
        if let Some(head) = &head {
            anyhow::ensure!(
                head.after.digest == record_digest(record)?,
                "observed review history gap: live Requirement differs from its recorded snapshot"
            );
        } else {
            anyhow::ensure!(
                record.schema_version != REVIEW_SCHEMA_VERSION,
                "enrolled Requirement has no review history"
            );
        }
        Ok(head)
    }

    pub(super) fn requirement(
        &self,
        scope: &ScopeId,
        id: &StableId,
    ) -> anyhow::Result<Requirement> {
        let records = self.list_requirements(scope)?;
        let matches = records.iter().filter(|r| r.id == *id).collect::<Vec<_>>();
        anyhow::ensure!(
            matches.len() == 1 && matches[0].scope_id == *scope,
            "Requirement does not exist uniquely in this scope"
        );
        Ok(matches[0].clone())
    }

    /// Full-scope import and export cannot yet carry review history.
    pub fn ensure_review_portable(&self, scope: &ScopeId) -> anyhow::Result<()> {
        self.with_repository_publication(|| {
            anyhow::ensure!(
                !directory(&self.layout, scope).try_exists()?
                    && !self
                        .list_requirements(scope)?
                        .iter()
                        .any(|r| r.schema_version == REVIEW_SCHEMA_VERSION),
                "review-bearing scopes require lossless import/export support"
            );
            Ok(())
        })
    }
}

pub(super) fn validated_head(entries: &[ReviewEntry]) -> anyhow::Result<Option<ReviewEntry>> {
    if entries.is_empty() {
        return Ok(None);
    }
    let mut previous: Option<&ReviewEntry> = None;
    let mut ids = std::collections::BTreeSet::new();
    let mut by_predecessor = std::collections::BTreeMap::<Option<&str>, Vec<&ReviewEntry>>::new();
    for entry in entries {
        by_predecessor
            .entry(entry.predecessor.as_ref().map(StableId::as_str))
            .or_default()
            .push(entry);
        anyhow::ensure!(
            ids.insert(entry.id.as_str()),
            "review conflict: duplicate entry identity"
        );
    }
    for index in 0..entries.len() {
        let successors = by_predecessor
            .get(&previous.map(|p| p.id.as_str()))
            .map_or(&[][..], Vec::as_slice);
        anyhow::ensure!(
            successors.len() == 1,
            "review conflict: missing or competing revision predecessors"
        );
        let next = successors[0];
        anyhow::ensure!(
            next.sequence == index as u64 + 1,
            "review conflict: invalid sequence"
        );
        if previous.is_none() {
            anyhow::ensure!(
                next.prior_revision.is_none(),
                "review conflict: initial revision has a predecessor"
            );
        }
        if let Some(prev) = previous {
            anyhow::ensure!(
                next.prior_revision.as_ref() == Some(&prev.revision)
                    && next.before.as_ref() == Some(&prev.after),
                "review conflict: evidence chain differs from its predecessor"
            );
        }
        previous = Some(next);
    }
    let head = previous.unwrap();
    anyhow::ensure!(
        !entries
            .iter()
            .any(|e| e.predecessor.as_ref() == Some(&head.id)),
        "review conflict: revision cycle"
    );
    Ok(Some(head.clone()))
}
