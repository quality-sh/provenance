//! What a history read found in the committed texts of one record. The
//! last of these is kept, with no text, so that a new read of the same
//! record over the same versions reads no text.

use super::origins::{self, Outcome};
use super::texts::each_text;
use super::versions::{FileVersion, FileVersions};
use super::{find, lineage, push_version, Source, VersionedRecord};
use provenance_core::{NodeType, StableId};
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, PoisonError};

/// The committed versions of one record, each with its outcomes.
pub(super) struct Committed {
    pub(super) versions: Vec<VersionedRecord>,
    /// The record at the last listed version of its file.
    pub(super) head: Option<Value>,
    /// The outcomes that the committed versions hold.
    pub(super) seen: BTreeSet<Outcome>,
}

/// The paths of the record file and of the Discussion file of a record.
pub(super) struct Paths<'a> {
    pub(super) records: &'a str,
    pub(super) discussions: &'a str,
}

struct Found {
    files: Arc<Vec<FileVersion>>,
    records: String,
    discussions: String,
    kind: NodeType,
    id: StableId,
    committed: Arc<Committed>,
}

// The committed versions of the last record read. The text of a file at a
// commit does not change, so the same list of versions gives the same
// records. An adapter gives the same list again when its history did not
// change; the git adapter does so for the same `HEAD`.
static LAST_FOUND: Mutex<Option<Found>> = Mutex::new(None);

/// The committed versions of the record `id` over the versions `files` of
/// its record file.
pub(super) async fn committed(
    history: &dyn FileVersions,
    files: &Arc<Vec<FileVersion>>,
    paths: Paths<'_>,
    kind: NodeType,
    id: &StableId,
) -> anyhow::Result<Arc<Committed>> {
    let known = LAST_FOUND
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .filter(|found| {
            Arc::ptr_eq(&found.files, files)
                && found.records == paths.records
                && found.discussions == paths.discussions
                && found.kind == kind
                && found.id == *id
        })
        .map(|found| found.committed.clone());
    if let Some(committed) = known {
        return Ok(committed);
    }
    // One text at a time: the read keeps only the record it finds in it.
    let mut records = Vec::with_capacity(files.len());
    let commits = files
        .iter()
        .map(|file| file.commit.as_str())
        .collect::<Vec<_>>();
    each_text(history, paths.records, &commits, |text| {
        records.push(text.as_deref().and_then(|text| find(text, id)));
    })
    .await?;
    let head = records.last().cloned().flatten();
    let mut versions = Vec::new();
    for index in lineage(files, &records) {
        let (file, Some(record)) = (&files[index], records[index].clone()) else {
            continue;
        };
        let source = Source {
            id: StableId::new(file.commit.clone())?,
            commit: Some(file.commit.clone()),
            author: Some(file.author.clone()),
            committed_at: Some(file.committed_at),
        };
        push_version(&mut versions, kind, source, record)?;
    }
    let mut seen = BTreeSet::new();
    origins::committed(
        history,
        paths.discussions,
        kind,
        id,
        &mut versions,
        &mut seen,
    )
    .await?;
    let committed = Arc::new(Committed {
        versions,
        head,
        seen,
    });
    *LAST_FOUND.lock().unwrap_or_else(PoisonError::into_inner) = Some(Found {
        files: files.clone(),
        records: paths.records.to_owned(),
        discussions: paths.discussions.to_owned(),
        kind,
        id: id.clone(),
        committed: committed.clone(),
    });
    Ok(committed)
}
