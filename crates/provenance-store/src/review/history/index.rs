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

use super::versions::FileVersion;
use crate::canonical_digest::{canonical_bytes, sha256};
use crate::operations::reader::ReadSnapshot;
use crate::read_database::{ReadDatabase, SqlColumns, SqlValue};
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

/// The index of one record file, as one history read of one record uses it.
#[derive(Default)]
pub struct Known {
    /// The base of each indexed commit.
    pub bases: BTreeMap<String, Option<String>>,
    /// The rows of the record that the read is for, by commit.
    pub record: BTreeMap<String, Option<Held>>,
    /// The content digest of each row of the file, by commit and record.
    /// The read loads them only when it indexes commits.
    pub digests: BTreeMap<String, BTreeMap<String, Option<String>>>,
}

/// The index rows that one history read made, for the read database.
#[derive(Default)]
pub struct Made {
    pub path: String,
    pub commits: Vec<(String, Option<String>)>,
    pub changes: Vec<(String, String, Option<Held>)>,
}

/// Loads the index of the record file `path` for the record `id`, over the
/// listed versions `files`.
pub async fn load(
    snapshot: &ReadSnapshot,
    path: &str,
    id: &StableId,
    files: &[FileVersion],
) -> anyhow::Result<Known> {
    let mut known = Known::default();
    let rows = snapshot
        .fetch(
            "SELECT commit_id, base FROM record_history_commits WHERE path = ?",
            vec![SqlValue::from(path)],
        )
        .await?;
    for row in &rows {
        let base = row.optional_text(1)?.map(str::to_owned);
        known.bases.insert(row.text(0)?.to_owned(), base);
    }
    let rows = snapshot
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
        known.record.insert(row.text(0)?.to_owned(), held);
    }
    let listed = files
        .iter()
        .map(|file| file.commit.as_str())
        .collect::<BTreeSet<_>>();
    let indexed = files
        .iter()
        .all(|file| known.bases.get(&file.commit) == Some(&base(file, &listed)));
    if !indexed {
        let rows = snapshot
            .fetch(
                "SELECT commit_id, record_id, digest FROM record_history_changes WHERE path = ?",
                vec![SqlValue::from(path)],
            )
            .await?;
        for row in &rows {
            known
                .digests
                .entry(row.text(0)?.to_owned())
                .or_default()
                .insert(
                    row.text(1)?.to_owned(),
                    row.optional_text(2)?.map(str::to_owned),
                );
        }
    }
    Ok(known)
}

/// Keeps the rows of `made` in `database`, in one write. The rows of a
/// commit replace the rows that the index held for it.
pub async fn save(database: &dyn ReadDatabase, made: Made) -> anyhow::Result<()> {
    if made.commits.is_empty() {
        return Ok(());
    }
    let mut statements = Vec::new();
    for (commit, base) in &made.commits {
        let key = vec![
            SqlValue::from(made.path.as_str()),
            SqlValue::from(commit.as_str()),
        ];
        statements.push((
            "DELETE FROM record_history_changes WHERE path = ? AND commit_id = ?",
            key.clone(),
        ));
        let mut values = key;
        values.push(base.as_deref().map_or(SqlValue::Null, SqlValue::from));
        statements.push((
            "INSERT OR REPLACE INTO record_history_commits (path, commit_id, base) VALUES (?, ?, ?)",
            values,
        ));
    }
    for (commit, record, held) in &made.changes {
        let mut values = vec![
            SqlValue::from(made.path.as_str()),
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
