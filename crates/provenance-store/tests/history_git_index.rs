//! The index of record changes over the git adapter, on each platform.
//!
//! A counting adapter wraps the git adapter, so the tests see how many
//! texts a read asks Git for. The reads of a record file with more than
//! 1,000 versions are evidence of `rule_history_reads_bound_file_versions`
//! and `rule_record_history_reads_index`.

mod discussion_support;

use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::review::ReviewHistoryQuery;
use provenance_core::NodeType;
use provenance_macros::verifies;
use provenance_store::operations::read_operations::{InProcessReadOperations, ReadOperations};
use provenance_store::operations::read_policy::ReadPolicy;
use provenance_store::operations::LocalRepository;
use provenance_store::repository::Repository;
use provenance_store::review::history::{BoxFuture, FileVersion, FileVersions, GitFileVersions};
use provenance_store::review::read_history;
use serde_json::json;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Adds `count` commits to the branch of `HEAD` with `git fast-import`, each
/// the child of the one before, that each give `req_a` in the Requirement
/// file a new statement. Then sets the work tree to the new `HEAD`.
fn statement_commits(temp: &tempfile::TempDir, count: usize) {
    let file = discussion_support::requirements(temp);
    let path = file
        .strip_prefix(camino::Utf8Path::from_path(temp.path()).unwrap())
        .unwrap()
        .as_str()
        .replace('\\', "/");
    let text = std::fs::read_to_string(&file).unwrap();
    let branch = discussion_support::git(temp, &["symbolic-ref", "HEAD"]);
    let head = discussion_support::git(temp, &["rev-parse", "HEAD"]);
    let mut stream = Vec::new();
    for index in 1..=count {
        let mut lines = String::new();
        for line in text.lines() {
            let mut record: serde_json::Value = serde_json::from_str(line).unwrap();
            if record["id"] == "req_a" {
                record["statement"] = format!("Edit {index} of the statement.").into();
            }
            writeln!(lines, "{record}").unwrap();
        }
        let from = if index == 1 {
            head.clone()
        } else {
            format!(":{}", index - 1)
        };
        stream.extend(
            format!(
                "commit {branch}\nmark :{index}\ncommitter Ann <ann@example.com> {} +0000\n\
                 data 4\nedit\nfrom {from}\nM 100644 inline {path}\ndata {}\n{lines}\n",
                1_700_000_000 + index,
                lines.len()
            )
            .into_bytes(),
        );
    }
    let mut child = provenance_store::git_process::git()
        .args(["fast-import", "--quiet"])
        .current_dir(temp.path())
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), &stream).unwrap();
    assert!(child.wait().unwrap().success());
    discussion_support::git(temp, &["reset", "-q", "--hard"]);
}

/// The git adapter of the repository at `temp`, with a count of its text
/// calls.
struct Counted {
    git: GitFileVersions,
    texts: AtomicUsize,
}

impl FileVersions for Counted {
    fn versions<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Arc<Vec<FileVersion>>>> {
        self.git.versions(path)
    }

    fn text<'a>(
        &'a self,
        path: &'a str,
        commit: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>> {
        self.texts.fetch_add(1, Ordering::SeqCst);
        self.git.text(path, commit)
    }
}

/// The repository at `temp`, opened again, with a new git adapter that
/// counts its text calls.
fn opened(temp: &tempfile::TempDir) -> (Repository, Arc<Counted>) {
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let counted = Arc::new(Counted {
        git: GitFileVersions::new(root.to_owned()),
        texts: AtomicUsize::new(0),
    });
    let repository = discussion_support::repository(root).with_file_versions(counted.clone());
    (repository, counted)
}

/// The history read of `req_a` in the repository at `temp`, opened again,
/// and the text calls of the read.
async fn read(temp: &tempfile::TempDir) -> (anyhow::Result<Vec<String>>, usize) {
    let query = ReviewHistoryQuery {
        record_kind: NodeType::Requirement,
        record_id: discussion_support::id(),
        limit: 200,
        cursor: None,
    };
    let (repository, counted) = opened(temp);
    let page = read_history(&repository, ReadPolicy::default(), query).await;
    let ids = page.map(|page| {
        page.result
            .entries
            .into_iter()
            .map(|entry| entry.id.as_str().to_owned())
            .collect()
    });
    (ids, counted.texts.load(Ordering::SeqCst))
}

/// A history read of a Git repository whose record file has more than 1,000
/// versions to index gives the budget refusal after it indexed 1,000, and
/// the next read answers. The index stays in the read database, so a read
/// in a repository opened again asks Git for no text. After `HEAD` moves
/// back one commit, a read lists no commit past `HEAD`.
#[tokio::test]
#[verifies("rule_history_reads_bound_file_versions", examples)]
#[verifies("rule_record_history_reads_index", examples)]
async fn a_read_of_more_than_1000_file_versions_indexes_them_over_two_reads() {
    let (temp, _store) = discussion_support::fixture();
    discussion_support::commit_state(&temp, "Create");
    statement_commits(&temp, 1_000);
    let (refused, calls) = read(&temp).await;
    let error = refused.expect_err("a refusal");
    assert_eq!(
        error.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::PageBudgetExceeded),
        "{error:#}"
    );
    assert_eq!(
        calls, 2_000,
        "1,000 record texts and 1,000 Discussion texts"
    );
    let head = discussion_support::git(&temp, &["rev-parse", "HEAD"]);
    let previous = discussion_support::git(&temp, &["rev-parse", "HEAD~1"]);
    let (first, calls) = read(&temp).await;
    let first = first.unwrap();
    assert_eq!(first.len(), 200);
    assert_eq!(calls, 2, "the last version");
    let (again, calls) = read(&temp).await;
    assert_eq!(again.unwrap(), first);
    assert_eq!(calls, 0, "a read of the indexed file");

    discussion_support::git(&temp, &["reset", "-q", "--hard", "HEAD~1"]);
    let entry = |id: &str| {
        let (repository, counted) = opened(&temp);
        let binding = LocalRepository::bind(repository, "folder").unwrap();
        let request = json!({
            "context": {"repository": "folder", "scope": "default"},
            "request": {"record_kind": "requirement", "record_id": "req_a", "entry_id": id},
        });
        async move {
            let answer = InProcessReadOperations::new(&binding)
                .run("review-history-entry", request)
                .await
                .map(|answer| answer["id"].clone())
                .map_err(|failure| failure.error);
            (answer, counted.texts.load(Ordering::SeqCst))
        }
    };
    assert_eq!(entry(&previous).await, (Ok(json!(previous)), 0));
    assert_eq!(
        entry(&head).await,
        (Err(json!({"kind": "resource_not_found"})), 0),
        "the commits are indexed"
    );
}
