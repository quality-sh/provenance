//! Enrolled records and their Message history require staged review writes.
use camino::Utf8Path;
use serde::Serialize;
use serde_json::Value;
use std::cell::RefCell;

thread_local! {
    static WRITERS: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn with_writer<R>(path: &Utf8Path, id: &str, run: impl FnOnce() -> R) -> R {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            WRITERS.with(|writers| {
                writers.borrow_mut().pop();
            });
        }
    }
    WRITERS.with(|writers| {
        writers
            .borrow_mut()
            .push((path.to_string(), id.to_string()));
    });
    let _reset = Reset;
    run()
}

pub(super) fn with_writers<R>(
    paths: &[camino::Utf8PathBuf],
    id: &str,
    run: impl FnOnce() -> R,
) -> R {
    struct Reset(usize);
    impl Drop for Reset {
        fn drop(&mut self) {
            WRITERS.with(|writers| writers.borrow_mut().truncate(self.0));
        }
    }
    let start = WRITERS.with(|writers| {
        let mut writers = writers.borrow_mut();
        let start = writers.len();
        writers.extend(paths.iter().map(|path| (path.to_string(), id.to_string())));
        start
    });
    let _reset = Reset(start);
    run()
}

pub fn writer_allows(path: &Utf8Path, id: &str) -> bool {
    WRITERS.with(|writers| {
        writers
            .borrow()
            .iter()
            .any(|(p, i)| p == path.as_str() && (i == id || i == "*"))
    })
}

pub fn writer_allows_path(path: &Utf8Path) -> bool {
    WRITERS.with(|writers| writers.borrow().iter().any(|(p, _)| p == path.as_str()))
}

pub fn protect_rows<T: Serialize>(path: &Utf8Path, records: &[T]) -> anyhow::Result<()> {
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
        if writer_allows(path, "*") {
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
        if !writer_allows(path, id) {
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

impl crate::state_store::StateStore {
    /// Full-scope import and export cannot yet carry review state.
    pub fn ensure_review_portable(&self, scope: &provenance_core::ScopeId) -> anyhow::Result<()> {
        self.with_repository_publication(|| {
            anyhow::ensure!(
                !self
                    .layout
                    .scopes_dir()
                    .join(scope.as_str())
                    .join("review")
                    .try_exists()?,
                "review-bearing scopes require lossless import/export support"
            );
            let has_review_state = !self.list_discussions(scope)?.is_empty()
                || !self.list_withdrawals(scope)?.is_empty()
                || self
                    .list_proposal_definitions(scope)?
                    .iter()
                    .any(|proposal| {
                        proposal.proposal_type == provenance_core::ProposalType::RecordRevision
                    });
            anyhow::ensure!(
                !has_review_state,
                "review-bearing scopes require lossless import/export support"
            );
            Ok(())
        })
    }
}
