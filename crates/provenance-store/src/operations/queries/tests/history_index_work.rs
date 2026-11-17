//! The work of the index load of a history read at a real size, against the
//! page work bound.
//!
//! Implementation aid with no Rule binding. The index of a record file of
//! 2,000 records with 10,000 commits that each change one record holds
//! 12,000 change rows and 10,000 commit rows. A read with one new commit
//! loads all of them. That load runs within a tenth of the engine's bound,
//! and a load that the bound stops gives the typed page budget refusal.

use super::{repository_at, root_of, seeded_store};
use crate::layout::ProvenanceLayout;
use crate::operations::read_policy::ReadPolicy;
use crate::read_database::SqlValue;
use crate::repository::Repository;
use crate::review::history::{BoxFuture, FileVersion, FileVersions};
use crate::review::read_history;
use crate::shards;
use crate::test_probes::set_page_work_steps;
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::review::{ReviewHistoryPage, ReviewHistoryQuery};
use provenance_core::{NodeType, StableId};
use std::sync::Arc;

const COMMITS: usize = 10_000;
const RECORDS: usize = 2_000;

/// A line of commits `c0`, `c1`, … of one record file, each the child of
/// the one before. Only the last commit has a text; the read asks for no
/// other.
#[derive(Debug)]
struct Line {
    path: String,
    files: Arc<Vec<FileVersion>>,
    last: String,
}

impl Line {
    fn new(path: &str, count: usize, last: &str) -> Arc<Self> {
        let files = (0..count)
            .map(|index| {
                let parents = index.checked_sub(1).map(|parent| format!("c{parent}"));
                FileVersion::new(
                    format!("c{index}"),
                    parents.into_iter().collect(),
                    "ann".to_owned(),
                    1_700_000_000,
                )
            })
            .collect();
        Arc::new(Self {
            path: path.to_owned(),
            files: Arc::new(files),
            last: last.to_owned(),
        })
    }
}

impl FileVersions for Line {
    fn versions<'a>(&'a self, _: &'a str) -> BoxFuture<'a, anyhow::Result<Arc<Vec<FileVersion>>>> {
        Box::pin(std::future::ready(Ok(self.files.clone())))
    }

    fn text<'a>(
        &'a self,
        path: &'a str,
        commit: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>> {
        let last = self.files.last().map(|file| file.commit.as_str());
        let text = (path == self.path && Some(commit) == last).then(|| self.last.clone());
        Box::pin(std::future::ready(Ok(text)))
    }
}

/// The history of `req_overtime` through `line`, under the work bound
/// `steps`, or the engine's bound when `None`.
async fn read(
    repository: &Repository,
    line: Arc<Line>,
    steps: Option<u64>,
) -> anyhow::Result<ReviewHistoryPage> {
    let opened = repository.with_file_versions(line);
    let query = ReviewHistoryQuery {
        record_kind: NodeType::Requirement,
        record_id: StableId::new("req_overtime").unwrap(),
        limit: 50,
        cursor: None,
    };
    set_page_work_steps(steps);
    let answer = read_history(&opened, ReadPolicy::default(), query).await;
    set_page_work_steps(None);
    Ok(answer?.result)
}

/// Writes the index of `path` for the commits `c0` to `c9999`: `c0` holds
/// each record, and each later commit changes one record.
async fn index_of_real_size(repository: &Repository, path: &str) {
    let mut statements = Vec::new();
    for index in 0..COMMITS {
        let base = index.checked_sub(1).map(|base| format!("c{base}"));
        statements.push((
            "INSERT INTO record_history_commits (path, commit_id, base) VALUES (?, ?, ?)",
            vec![
                SqlValue::from(path),
                SqlValue::from(format!("c{index}").as_str()),
                base.as_deref().map_or(SqlValue::Null, SqlValue::from),
            ],
        ));
        let records = if index == 0 {
            (0..RECORDS).collect::<Vec<_>>()
        } else {
            vec![index % RECORDS]
        };
        for record in records {
            statements.push((
                "INSERT INTO record_history_changes \
                 (path, commit_id, record_id, digest, revision, fields, outcomes) \
                 VALUES (?, ?, ?, ?, 'rev_x', '{}', '[]')",
                vec![
                    SqlValue::from(path),
                    SqlValue::from(format!("c{index}").as_str()),
                    SqlValue::from(format!("req_{record}").as_str()),
                    SqlValue::from(format!("digest {index}").as_str()),
                ],
            ));
        }
    }
    repository
        .read_database()
        .unwrap()
        .write(Box::new(move |mut transaction| {
            Box::pin(async move {
                for (sql, values) in statements {
                    transaction.execute(sql, values).await?;
                }
                transaction.commit().await
            })
        }))
        .await
        .unwrap();
}

#[tokio::test]
async fn the_index_load_of_a_real_size_runs_within_the_page_work_bound() {
    let (dir, _store, scope) = seeded_store();
    let root = root_of(&dir);
    let repository = repository_at(&root);
    let layout = ProvenanceLayout::new(root.clone());
    let shard = shards::path_for(&layout, &scope, NodeType::Requirement);
    let path = shard
        .strip_prefix(&root)
        .unwrap()
        .as_str()
        .replace('\\', "/");
    let saved = std::fs::read_to_string(&shard).unwrap();
    read(&repository, Line::new(&path, 0, ""), None)
        .await
        .unwrap();
    index_of_real_size(&repository, &path).await;

    let page = read(
        &repository,
        Line::new(&path, COMMITS + 1, &saved),
        Some(1_000_000),
    )
    .await;
    let page = page.unwrap_or_else(|error| panic!("a tenth of the bound: {error:#}"));
    assert_eq!(page.entries[0].id.as_str(), format!("c{COMMITS}"));

    let longer = || Line::new(&path, COMMITS + 2, &saved.replace("Overtime", "Pay"));
    let refused = read(&repository, longer(), Some(0)).await.unwrap_err();
    assert_eq!(
        refused.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::PageBudgetExceeded),
        "{refused:#}"
    );
    let page = read(&repository, longer(), None).await.unwrap();
    let ids = page.entries.iter().map(|entry| entry.id.as_str());
    let ids = ids.collect::<Vec<_>>();
    assert_eq!(
        ids,
        [
            &format!("c{COMMITS}"),
            &format!("c{}", COMMITS + 1),
            "working"
        ]
    );
}
