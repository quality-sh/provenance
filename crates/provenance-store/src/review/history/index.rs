//! The index of the record changes of each record file, kept in the read
//! database.
//!
//! For each listed commit of a record file that a history read indexed,
//! the index holds the base of the commit, its first listed parent, and one
//! row for each record whose content at the commit differs from its content
//! at the base. A row holds the digest of each field of the record, its
//! revision and its Discussion outcomes at the commit, or nothing when the
//! file does not hold the record. So a history read of an indexed file
//! reads no committed text. The index is a cache: a rebuild of the read
//! database removes it, and the next history reads make it again from the
//! committed texts.
//!
//! The index tables are projection tables: a read takes them through the
//! [`ReadSnapshot::record_history`] handle, which puts `record_history` in
//! the attested list of the stamp. A write of index rows writes no
//! projection revision.

use super::versions::FileVersion;
use crate::canonical_digest::{canonical_bytes, sha256};
use crate::operations::reader::{page_error, ReadSnapshot};
use crate::read_database::{ReadDatabase, SqlColumns, SqlRow, SqlValue};
use crate::review::classifier;
use provenance_core::{NodeType, StableId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// What a record holds at one commit.
#[derive(Clone, Debug)]
pub struct Held {
    /// The digest of the record content: each field except its stamps.
    pub digest: String,
    pub revision: StableId,
    /// The digest of the value of each field, stamps included.
    pub fields: BTreeMap<String, String>,
    /// The outcomes of the Discussion file at the commit for the record,
    /// in file order.
    pub outcomes: Vec<OutcomeRow>,
}

/// One Discussion outcome for a record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutcomeRow {
    pub discussion_id: StableId,
    pub thread_id: StableId,
    pub message_id: StableId,
    pub revision: String,
}

/// The record stamps, which are not record content.
const STAMPS: [&str; 2] = ["created", "updated"];

impl Held {
    /// What `record` holds, with no outcomes.
    pub fn of(kind: NodeType, record: &Value) -> anyhow::Result<Self> {
        let mut fields = BTreeMap::new();
        for (name, value) in record.as_object().into_iter().flatten() {
            fields.insert(name.clone(), sha256(&canonical_bytes(value)?));
        }
        let content = fields
            .iter()
            .filter(|(name, _)| !STAMPS.contains(&name.as_str()))
            .collect::<BTreeMap<_, _>>();
        Ok(Self {
            digest: sha256(&canonical_bytes(&content)?),
            revision: classifier::review_revision(kind, record)?,
            fields,
            outcomes: Vec::new(),
        })
    }

    /// The digest of each content field: each field except the stamps.
    pub fn content(&self) -> impl Iterator<Item = (&String, &String)> {
        self.fields
            .iter()
            .filter(|(name, _)| !STAMPS.contains(&name.as_str()))
    }
}

/// The base of a listed commit: its first parent that the list holds.
pub fn base(file: &FileVersion, listed: &BTreeSet<&str>) -> Option<String> {
    file.parents
        .iter()
        .find(|parent| listed.contains(parent.as_str()))
        .cloned()
}

/// The commits of `files` that the list holds.
fn listed(files: &[FileVersion]) -> BTreeSet<&str> {
    files.iter().map(|file| file.commit.as_str()).collect()
}

/// The index of one record file, as one history read of one record uses
/// it, with the rows that the read adds to it.
#[derive(Default)]
pub struct RecordIndex {
    path: String,
    /// The base of each indexed commit.
    bases: BTreeMap<String, Option<String>>,
    /// The rows of the record that the read is for, by commit.
    record: BTreeMap<String, Option<Held>>,
    /// The content digest of each row of the file, by commit and record.
    /// The read loads them only when commits wait.
    digests: BTreeMap<String, BTreeMap<String, Option<String>>>,
    /// The commits that the read indexed, each with its base.
    made: Vec<(String, Option<String>)>,
    /// The rows that the read made: commit, record and what it holds.
    changes: Vec<(String, String, Option<Held>)>,
}

/// The index tables of one read snapshot.
pub struct RecordHistory<'s> {
    snapshot: &'s ReadSnapshot,
}

impl ReadSnapshot {
    /// A handle on the index tables; taking it puts `record_history` in
    /// `attested`.
    pub fn record_history(&self) -> RecordHistory<'_> {
        self.attest("record_history");
        RecordHistory { snapshot: self }
    }
}

impl RecordHistory<'_> {
    /// Runs one statement on the index tables. A statement that the page
    /// work bound stops gives the page budget refusal.
    async fn fetch(&self, sql: &str, values: Vec<SqlValue>) -> anyhow::Result<Vec<SqlRow>> {
        self.snapshot.fetch(sql, values).await.map_err(page_error)
    }
}

impl RecordIndex {
    /// Loads the index of the record file `path` for the record `id`, over
    /// the listed versions `files`.
    pub async fn load(
        snapshot: &ReadSnapshot,
        path: &str,
        id: &StableId,
        files: &[FileVersion],
    ) -> anyhow::Result<Self> {
        let tables = snapshot.record_history();
        let mut index = Self {
            path: path.to_owned(),
            ..Self::default()
        };
        let rows = tables
            .fetch(
                "SELECT commit_id, base FROM record_history_commits WHERE path = ?",
                vec![SqlValue::from(path)],
            )
            .await?;
        for row in &rows {
            let base = row.optional_text(1)?.map(str::to_owned);
            index.bases.insert(row.text(0)?.to_owned(), base);
        }
        let rows = tables
            .fetch(
                "SELECT commit_id, digest, revision, fields, outcomes FROM record_history_changes \
                 WHERE path = ? AND record_id = ?",
                vec![SqlValue::from(path), SqlValue::from(id.as_str())],
            )
            .await?;
        for row in &rows {
            let held = match row.optional_text(1)? {
                None => None,
                Some(digest) => Some(Held {
                    digest: digest.to_owned(),
                    revision: StableId::new(row.text(2)?)?,
                    fields: serde_json::from_str(row.text(3)?)?,
                    outcomes: serde_json::from_str(row.text(4)?)?,
                }),
            };
            index.record.insert(row.text(0)?.to_owned(), held);
        }
        if !index.waiting(files).is_empty() {
            let rows = tables
                .fetch(
                    "SELECT commit_id, record_id, digest FROM record_history_changes WHERE path = ?",
                    vec![SqlValue::from(path)],
                )
                .await?;
            for row in &rows {
                index
                    .digests
                    .entry(row.text(0)?.to_owned())
                    .or_default()
                    .insert(
                        row.text(1)?.to_owned(),
                        row.optional_text(2)?.map(str::to_owned),
                    );
            }
        }
        Ok(index)
    }

    /// The path of the record file.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The listed commits that the index does not hold at the base that the
    /// list gives them, in list order, each with that base.
    pub fn waiting<'f>(&self, files: &'f [FileVersion]) -> Vec<(&'f FileVersion, Option<String>)> {
        let listed = listed(files);
        files
            .iter()
            .map(|file| (file, base(file, &listed)))
            .filter(|(file, base)| self.bases.get(&file.commit) != Some(base))
            .collect()
    }

    /// What the record of the read holds at each listed commit: its row,
    /// else what it holds at the base. The index holds each listed commit.
    pub fn states<'i>(&'i self, files: &[FileVersion]) -> Vec<Option<&'i Held>> {
        let listed = listed(files);
        let mut states: Vec<Option<&Held>> = Vec::with_capacity(files.len());
        let mut at = BTreeMap::new();
        for (index, file) in files.iter().enumerate() {
            let state = self.record.get(&file.commit).map_or_else(
                || {
                    base(file, &listed)
                        .and_then(|base| at.get(&base).copied())
                        .and_then(|index: usize| states[index])
                },
                Option::as_ref,
            );
            states.push(state);
            at.insert(file.commit.clone(), index);
        }
        states
    }

    /// The content digest of each record at `commit`, an indexed commit,
    /// from the rows on the walk from it through each base.
    pub fn stored(&self, commit: &str) -> BTreeMap<String, String> {
        let mut found = BTreeMap::new();
        let mut at = Some(commit.to_owned());
        while let Some(commit) = at {
            for (record, digest) in self.digests.get(&commit).into_iter().flatten() {
                found
                    .entry(record.clone())
                    .or_insert_with(|| digest.clone());
            }
            at = self.bases.get(&commit).cloned().flatten();
        }
        found
            .into_iter()
            .filter_map(|(record, digest)| Some((record, digest?)))
            .collect()
    }

    /// Puts the rows `changes` of the listed commit `commit` at `base` in
    /// the index, in place of the rows that it held for the commit. `id` is
    /// the record of the read.
    pub fn put(
        &mut self,
        commit: &str,
        base: Option<String>,
        id: &StableId,
        changes: Vec<(String, Option<Held>)>,
    ) {
        self.digests.remove(commit);
        self.record.remove(commit);
        for (record, held) in changes {
            self.digests.entry(commit.to_owned()).or_default().insert(
                record.clone(),
                held.as_ref().map(|held| held.digest.clone()),
            );
            if record == id.as_str() {
                self.record.insert(commit.to_owned(), held.clone());
            }
            self.changes.push((commit.to_owned(), record, held));
        }
        self.bases.insert(commit.to_owned(), base.clone());
        self.made.push((commit.to_owned(), base));
    }

    /// Keeps the rows that the read made in `database`, in one write. The
    /// rows of a commit replace the rows that the index held for it.
    pub async fn save(self, database: &dyn ReadDatabase) -> anyhow::Result<()> {
        if self.made.is_empty() {
            return Ok(());
        }
        let mut statements = Vec::new();
        for (commit, base) in &self.made {
            let key = vec![
                SqlValue::from(self.path.as_str()),
                SqlValue::from(commit.as_str()),
            ];
            statements.push((
                "DELETE FROM record_history_changes WHERE path = ? AND commit_id = ?",
                key.clone(),
            ));
            let mut values = key;
            values.push(base.as_deref().map_or(SqlValue::Null, SqlValue::from));
            statements.push((
                "INSERT OR REPLACE INTO record_history_commits (path, commit_id, base) \
                 VALUES (?, ?, ?)",
                values,
            ));
        }
        for (commit, record, held) in &self.changes {
            let mut values = vec![
                SqlValue::from(self.path.as_str()),
                SqlValue::from(commit.as_str()),
                SqlValue::from(record.as_str()),
            ];
            match held {
                None => values.extend([
                    SqlValue::Null,
                    SqlValue::Null,
                    SqlValue::Null,
                    SqlValue::Null,
                ]),
                Some(held) => values.extend([
                    SqlValue::from(held.digest.as_str()),
                    SqlValue::from(held.revision.as_str()),
                    SqlValue::Text(serde_json::to_string(&held.fields)?),
                    SqlValue::Text(serde_json::to_string(&held.outcomes)?),
                ]),
            }
            statements.push((
                "INSERT OR REPLACE INTO record_history_changes \
                 (path, commit_id, record_id, digest, revision, fields, outcomes) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                values,
            ));
        }
        database
            .write(Box::new(move |mut transaction| {
                Box::pin(async move {
                    for (sql, values) in statements {
                        transaction.execute(sql, values).await?;
                    }
                    transaction.commit().await
                })
            }))
            .await
    }
}
