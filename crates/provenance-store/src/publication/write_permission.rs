//! Write permissions for enrolled records and their Message history.
//!
//! A permission belongs to one staged store. A review writer gets a store
//! with the added permission from `StateStore::permitting`, and a write
//! through any other store carries none.
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;
use serde::Serialize;
use serde_json::Value;

/// The shards a staged store may change enrolled rows in, each for one
/// record id or for every id (`*`).
#[derive(Clone, Debug, Default)]
pub(crate) struct WritePermissions {
    grants: Vec<(Utf8PathBuf, String)>,
}

impl WritePermissions {
    /// No permission. Raw writers and stores outside a review write use it.
    pub(crate) const fn none() -> Self {
        Self { grants: Vec::new() }
    }

    /// These permissions and one more for each of `paths`.
    pub(crate) fn with(&self, paths: &[&Utf8Path], id: &str) -> Self {
        let mut grants = self.grants.clone();
        grants.extend(paths.iter().map(|path| ((*path).to_owned(), id.to_owned())));
        Self { grants }
    }

    pub(crate) fn allows(&self, path: &Utf8Path, id: &str) -> bool {
        self.grants
            .iter()
            .any(|(p, i)| p == path && (i == id || i == "*"))
    }

    pub(crate) fn allows_path(&self, path: &Utf8Path) -> bool {
        self.grants.iter().any(|(p, _)| p == path)
    }
}

#[rule("rule_enrolled_record_write_needs_store_permission")]
pub(crate) fn protect_rows<T: Serialize>(
    path: &Utf8Path,
    records: &[T],
    permissions: &WritePermissions,
) -> anyhow::Result<()> {
    let directory = path.parent().and_then(Utf8Path::file_name);
    let family = directory.and_then(crate::cache::review_families::by_directory);
    if family.is_none() && directory != Some("threads") {
        return Ok(());
    }
    let before: Vec<Value> = match std::fs::read_to_string(path) {
        Ok(text) => text
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    let after = records
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    let discussions = if directory == Some("threads") {
        let discussion_path = path.parent().unwrap().join("discussions.jsonl");
        match std::fs::read_to_string(&discussion_path) {
            Ok(text) => text
                .lines()
                .map(serde_json::from_str::<Value>)
                .collect::<Result<Vec<_>, _>>()?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error.into()),
        }
    } else {
        Vec::new()
    };
    if directory == Some("threads") && path.file_name() != Some("threads.jsonl") {
        if permissions.allows(path, "*") {
            return Ok(());
        }
        for message in before.iter().chain(&after) {
            if discussions
                .iter()
                .any(|discussion| discussion["thread_id"] == message["thread_id"])
            {
                anyhow::ensure!(
                    before
                        .iter()
                        .filter(|m| m["id"] == message["id"])
                        .collect::<Vec<_>>()
                        == after
                            .iter()
                            .filter(|m| m["id"] == message["id"])
                            .collect::<Vec<_>>(),
                    "enrolled Thread requires an addressed Discussion write"
                );
            }
        }
    }
    let submitted = if let Some(family) = family {
        submitted_ids(path, family.kind)?
    } else {
        std::collections::BTreeSet::new()
    };
    for record in &before {
        if family.is_some()
            && !record["id"]
                .as_str()
                .is_some_and(|id| submitted.contains(id))
        {
            continue;
        }
        if family.is_none()
            && !discussions
                .iter()
                .any(|discussion| discussion["thread_id"] == record["id"])
        {
            continue;
        }
        let owner_field = family.map_or("id", |facts| facts.owner_field);
        let id = record[owner_field].as_str().unwrap_or_default();
        if !permissions.allows(path, id) {
            let old = before.iter().filter(|r| r["id"] == id).collect::<Vec<_>>();
            let new = after.iter().filter(|r| r["id"] == id).collect::<Vec<_>>();
            anyhow::ensure!(
                old.len() == 1 && new.len() == 1 && old == new,
                "enrolled record {id} requires a guarded review save"
            );
        }
    }
    Ok(())
}

fn submitted_ids(
    path: &Utf8Path,
    kind: provenance_core::NodeType,
) -> anyhow::Result<std::collections::BTreeSet<String>> {
    let Some(scope) = path.parent().and_then(Utf8Path::parent) else {
        return Ok(std::collections::BTreeSet::new());
    };
    let text = match std::fs::read_to_string(scope.join("ideation/proposal_cards.jsonl")) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(std::collections::BTreeSet::new())
        }
        Err(error) => return Err(error.into()),
    };
    let mut ids = std::collections::BTreeSet::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let proposal: Value = serde_json::from_str(line)?;
        let target = &proposal["traceability"]["target"];
        if proposal["proposal_type"] == "record_revision"
            && target["artifact_type"] == kind.as_str()
        {
            if let Some(id) = target["artifact_id"].as_str() {
                ids.insert(id.to_owned());
            }
        }
    }
    Ok(ids)
}
