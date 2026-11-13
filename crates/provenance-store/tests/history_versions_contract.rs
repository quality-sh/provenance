//! The file versions contract on the git adapter and on the memory adapter,
//! and record history through the file versions that a repository is given.
//!
//! No Rule: the plan proposes `rule_record_history_adapters_agree` for the
//! owner's review. Until it is approved, these tests hold the contract: the
//! git adapter shows that the expected answers are Git's, and the memory
//! adapter shows that an adapter with no Git gives the same answers.
#![cfg(feature = "host")]

#[allow(dead_code)]
mod review_support;

use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::review::ReviewHistoryQuery;
use provenance_core::NodeType;
use provenance_store::operations::read_policy::ReadPolicy;
use provenance_store::review::history::contract::{
    self, Adapter, Commit, Harness, MemoryFileVersions,
};
use provenance_store::review::history::GitFileVersions;
use provenance_store::review::read_history;
use std::collections::BTreeMap;
use std::io::Write;
use std::process::Stdio;
use std::sync::Arc;

/// A Git repository that holds the commits, made with Git's plumbing so
/// that each commit has exactly the parents and files of the graph.
struct Git {
    temps: std::sync::Mutex<Vec<tempfile::TempDir>>,
}

fn git(
    root: &std::path::Path,
    env: &[(&str, String)],
    args: &[&str],
    input: Option<&str>,
) -> String {
    let mut command = provenance_store::git_process::git();
    command
        .args(args)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in env {
        command.env(name, value);
    }
    let mut child = command.spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin
        .write_all(input.unwrap_or_default().as_bytes())
        .unwrap();
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "git {args:?} failed: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

impl Harness for Git {
    fn adapter(&self, commits: &[Commit]) -> Adapter {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        git(root, &[], &["init", "-q"], None);
        let mut ids: BTreeMap<&'static str, String> = BTreeMap::new();
        for commit in commits {
            match commit.parents.first() {
                Some(first) => git(root, &[], &["read-tree", &ids[first]], None),
                None => git(root, &[], &["read-tree", "--empty"], None),
            };
            for (path, text) in commit.changes {
                match text {
                    Some(text) => {
                        let blob = git(root, &[], &["hash-object", "-w", "--stdin"], Some(*text));
                        let entry = format!("100644,{blob},{path}");
                        git(
                            root,
                            &[],
                            &["update-index", "--add", "--cacheinfo", &entry],
                            None,
                        );
                    }
                    None => {
                        git(root, &[], &["update-index", "--force-remove", *path], None);
                    }
                }
            }
            let tree = git(root, &[], &["write-tree"], None);
            let mut args = vec!["commit-tree".to_owned(), tree];
            for parent in commit.parents {
                args.push("-p".to_owned());
                args.push(ids[parent].clone());
            }
            args.extend(["-m".to_owned(), commit.name.to_owned()]);
            let date = format!("{} +0000", commit.committed_at);
            let env = [
                ("GIT_AUTHOR_NAME", commit.author.to_owned()),
                ("GIT_AUTHOR_EMAIL", format!("{}@example.com", commit.author)),
                ("GIT_COMMITTER_NAME", commit.author.to_owned()),
                (
                    "GIT_COMMITTER_EMAIL",
                    format!("{}@example.com", commit.author),
                ),
                ("GIT_AUTHOR_DATE", date.clone()),
                ("GIT_COMMITTER_DATE", date),
            ];
            let args = args.iter().map(String::as_str).collect::<Vec<_>>();
            ids.insert(commit.name, git(root, &env, &args, None));
        }
        let head = &ids[commits.last().unwrap().name];
        git(root, &[], &["update-ref", "refs/heads/main", head], None);
        git(
            root,
            &[],
            &["symbolic-ref", "HEAD", "refs/heads/main"],
            None,
        );
        let root = Utf8PathBuf::from_path_buf(dunce::canonicalize(root).unwrap()).unwrap();
        self.temps.lock().unwrap().push(temp);
        Adapter {
            versions: Arc::new(GitFileVersions::new(root)),
            ids,
        }
    }
}

fn git_harness() -> Git {
    Git {
        temps: std::sync::Mutex::new(Vec::new()),
    }
}

struct Memory;

impl Harness for Memory {
    fn adapter(&self, commits: &[Commit]) -> Adapter {
        contract::memory_adapter(commits)
    }
}

#[tokio::test]
async fn git_versions_follow_the_history_of_head() {
    contract::versions_follow_the_history_of_head(&git_harness()).await;
}

#[tokio::test]
async fn git_texts_give_the_file_at_each_commit() {
    contract::texts_give_the_file_at_each_commit(&git_harness()).await;
}

#[tokio::test]
async fn git_a_file_that_no_commit_holds_has_no_versions() {
    contract::a_file_that_no_commit_holds_has_no_versions(&git_harness()).await;
}

#[tokio::test]
async fn memory_versions_follow_the_history_of_head() {
    contract::versions_follow_the_history_of_head(&Memory).await;
}

#[tokio::test]
async fn memory_texts_give_the_file_at_each_commit() {
    contract::texts_give_the_file_at_each_commit(&Memory).await;
}

#[tokio::test]
async fn memory_a_file_that_no_commit_holds_has_no_versions() {
    contract::a_file_that_no_commit_holds_has_no_versions(&Memory).await;
}

fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// Implementation aid with no Rule binding: a repository that is given file
/// versions reads its record history through them and not through Git. The
/// fixture folder has no Git history, so a read through Git lists only the
/// working copy.
#[tokio::test]
async fn a_repository_reads_record_history_through_its_file_versions() {
    let (temp, _store) = review_support::fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let repository = review_support::repository(root);
    let shard = provenance_store::shards::path_for(
        repository.layout(),
        &review_support::scope(),
        NodeType::Requirement,
    );
    let working = leak(std::fs::read_to_string(&shard).unwrap());
    let earlier = leak(working.replace("The system stores records.", "The system kept records."));
    let path = leak(
        shard
            .strip_prefix(repository.working_tree())
            .unwrap()
            .as_str()
            .replace('\\', "/"),
    );
    let commits = vec![
        Commit {
            name: "first",
            parents: &[],
            author: "ann",
            committed_at: 1_700_000_100,
            changes: Box::leak(vec![(path, Some(earlier))].into_boxed_slice()),
        },
        Commit {
            name: "second",
            parents: &["first"],
            author: "bea",
            committed_at: 1_700_000_200,
            changes: Box::leak(vec![(path, Some(working))].into_boxed_slice()),
        },
    ];
    let repository = repository.with_file_versions(Arc::new(MemoryFileVersions::new(&commits)));
    let query = ReviewHistoryQuery {
        record_kind: NodeType::Requirement,
        record_id: review_support::id(),
        limit: 10,
        cursor: None,
    };
    let entries = read_history(&repository, ReadPolicy::default(), query)
        .await
        .unwrap()
        .result
        .entries;
    let ids = entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["first", "second"]);
    assert_eq!(entries[1].author.as_deref(), Some("bea"));
    assert_eq!(entries[1].committed_at, Some(1_700_000_200));
    assert_eq!(entries[1].changed_fields, ["statement"]);
}
