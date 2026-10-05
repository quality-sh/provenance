use crate::{canonical_digest, layout::ProvenanceLayout, state_store::StateStore};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::review::{CycleEntry, JournalEntry, ReviewRecord, REVIEW_SCHEMA_VERSION};
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

pub(super) fn read_journal_entry(
    layout: &ProvenanceLayout,
    path: &Utf8Path,
) -> anyhow::Result<JournalEntry> {
    let entry: JournalEntry = read_bounded(layout, path, ENTRY_BYTES)?;
    let JournalEntry::Cycle(cycle) = &entry;
    let version = cycle.schema_version;
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

pub(super) fn new_id() -> StableId {
    StableId::new(uuid::Uuid::new_v4().to_string()).expect("UUID uses valid stable ID characters")
}

pub(super) fn etag(record: &ReviewRecord) -> anyhow::Result<String> {
    let content = provenance_core::model::record_stamps::content_value(record)?;
    Ok(canonical_digest::digest(
        &canonical_digest::canonical_bytes(&content)?,
    ))
}

impl StateStore {
    pub(super) fn cycle_entries(&self, scope: &ScopeId) -> anyhow::Result<Vec<CycleEntry>> {
        Ok(self
            .journal_entries(scope)?
            .into_iter()
            .map(|JournalEntry::Cycle(e)| *e)
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

    pub(super) fn requirement(
        &self,
        scope: &ScopeId,
        id: &StableId,
    ) -> anyhow::Result<Requirement> {
        let records = self.list_requirements(scope)?;
        let matches = records.iter().filter(|r| r.id == *id).collect::<Vec<_>>();
        anyhow::ensure!(
            matches.len() == 1 && matches[0].scope_id == *scope,
            "requirement {} does not exist uniquely in this scope",
            id.as_str()
        );
        Ok(matches[0].clone())
    }

    /// Full-scope import and export cannot yet carry review history.
    pub fn ensure_review_portable(&self, scope: &ScopeId) -> anyhow::Result<()> {
        self.with_repository_publication(|| {
            anyhow::ensure!(
                !directory(&self.layout, scope).try_exists()?,
                "review-bearing scopes require lossless import/export support"
            );
            let has_enrolled_record =
                crate::cache::review_families::has_enrolled_record(self, scope)?;
            anyhow::ensure!(
                !has_enrolled_record,
                "review-bearing scopes require lossless import/export support"
            );
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::etag;
    use provenance_core::Requirement;
    use serde_json::json;

    /// Implementation aid: pins that the etag ignores record stamps; no Rule
    /// names it.
    #[test]
    fn requirement_etag_ignores_record_stamps() {
        let value = json!({
            "schema_version": 2,
            "scope_id": "default",
            "id": "req_stamp",
            "statement": "The system stores records.",
            "status": "active"
        });
        let before: Requirement = serde_json::from_value(value.clone()).unwrap();
        let mut after: Requirement = serde_json::from_value(value).unwrap();
        after.created = Some(
            serde_json::from_value(json!({
                "commit": "a".repeat(40),
                "at": "2026-09-12T00:00:00Z"
            }))
            .unwrap(),
        );
        after.updated = Some(
            serde_json::from_value(json!({
                "commit": "b".repeat(40),
                "at": "2026-09-12T01:00:00Z"
            }))
            .unwrap(),
        );

        assert_eq!(etag(&before.into()).unwrap(), etag(&after.into()).unwrap());
    }
}
