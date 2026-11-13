//! The file versions contract on the git adapter and on the memory adapter,
//! and record history through the file versions that a repository is given.
//!
//! No Rule: the plan proposes `rule_record_history_adapters_agree` for the
//! owner's review. Until it is approved, these tests hold the contract: the
//! git adapter shows that the expected answers are Git's, and the memory
//! adapter shows that an adapter with no Git gives the same answers.

mod discussion_support;
#[path = "support/memory.rs"]
mod memory;

use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::review::{RecordVersion, ReviewHistoryQuery};
use provenance_core::threads::DiscussionOrigin;
use provenance_core::{NodeType, StableId};
use provenance_store::operations::read_policy::ReadPolicy;
use provenance_store::repository::Repository;
use provenance_store::review::history::contract::{
    self, Adapter, Commit, Harness, MemoryFileVersions,
};
use provenance_store::review::history::GitFileVersions;
use provenance_store::review::history::{BoxFuture, FileVersion, FileVersions};
use provenance_store::review::read_history;
use provenance_store::shards;
use serde_json::json;
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
        let mut ids: BTreeMap<String, String> = BTreeMap::new();
        for commit in commits {
            let base = commit
                .parents
                .first()
                .map_or("--empty", |first| ids[first.as_str()].as_str());
            git(root, &[], &["read-tree", base], None);
            for (path, text) in &commit.changes {
                match text {
                    Some(text) => {
                        let blob = git(root, &[], &["hash-object", "-w", "--stdin"], Some(text));
                        let entry = format!("100644,{blob},{path}");
                        git(
                            root,
                            &[],
                            &["update-index", "--add", "--cacheinfo", &entry],
                            None,
                        );
                    }
                    None => {
                        git(root, &[], &["update-index", "--force-remove", path], None);
                    }
                }
            }
            let tree = git(root, &[], &["write-tree"], None);
            let mut args = vec!["commit-tree".to_owned(), tree];
            for parent in &commit.parents {
                args.push("-p".to_owned());
                args.push(ids[parent.as_str()].clone());
            }
            args.extend(["-m".to_owned(), commit.name.clone()]);
            let date = format!("{} +0000", commit.committed_at);
            let env = [
                ("GIT_AUTHOR_NAME", commit.author.clone()),
                ("GIT_AUTHOR_EMAIL", format!("{}@example.com", commit.author)),
                ("GIT_COMMITTER_NAME", commit.author.clone()),
                (
                    "GIT_COMMITTER_EMAIL",
                    format!("{}@example.com", commit.author),
                ),
                ("GIT_AUTHOR_DATE", date.clone()),
                ("GIT_COMMITTER_DATE", date),
            ];
            let args = args.iter().map(String::as_str).collect::<Vec<_>>();
            ids.insert(commit.name.clone(), git(root, &env, &args, None));
        }
        let head = &ids[commits.last().unwrap().name.as_str()];
        git(root, &[], &["update-ref", "refs/heads/main", head], None);
        git(
            root,
            &[],
            &["symbolic-ref", "HEAD", "refs/heads/main"],
            None,
        );
        let root = Utf8PathBuf::from_path_buf(dunce::canonicalize(root).unwrap()).unwrap();
        self.temps.lock().unwrap().push(temp);
        Adapter::new(Arc::new(GitFileVersions::new(root)), ids)
    }
}

const fn git_harness() -> Git {
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

/// An adapter that compares a merge with its first parent only: it lists a
/// merge that keeps the file of its second parent. It stands for an adapter
/// that the contract must refuse.
#[derive(Debug)]
struct FirstParentOnly {
    commits: Vec<Commit>,
    files: Vec<BTreeMap<String, String>>,
}

impl FirstParentOnly {
    fn new(commits: &[Commit]) -> Self {
        let mut files: Vec<BTreeMap<String, String>> = Vec::new();
        for commit in commits {
            let mut file = commit.parents.first().map_or_else(BTreeMap::new, |first| {
                files[Self::at(commits, first)].clone()
            });
            for (path, text) in &commit.changes {
                match text {
                    Some(text) => file.insert(path.clone(), text.clone()),
                    None => file.remove(path),
                };
            }
            files.push(file);
        }
        Self {
            commits: commits.to_vec(),
            files,
        }
    }

    fn at(commits: &[Commit], name: &str) -> usize {
        commits
            .iter()
            .position(|commit| commit.name == name)
            .unwrap()
    }

    /// The nearest listed commits at `index`, which `listed` gets.
    fn nearest(
        &self,
        index: usize,
        path: &str,
        listed: &mut Vec<(usize, Vec<usize>)>,
    ) -> Vec<usize> {
        let parents = self.commits[index]
            .parents
            .iter()
            .map(|parent| Self::at(&self.commits, parent))
            .collect::<Vec<_>>();
        let text = self.files[index].get(path);
        if parents.is_empty() && text.is_none() {
            return Vec::new();
        }
        if parents
            .first()
            .is_some_and(|first| self.files[*first].get(path) == text)
        {
            return self.nearest(parents[0], path, listed);
        }
        if !listed.iter().any(|(known, _)| *known == index) {
            let lines = parents
                .iter()
                .flat_map(|parent| self.nearest(*parent, path, listed))
                .collect();
            listed.push((index, lines));
        }
        vec![index]
    }
}

impl FileVersions for FirstParentOnly {
    fn versions<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Arc<Vec<FileVersion>>>> {
        let mut listed = Vec::new();
        self.nearest(self.commits.len() - 1, path, &mut listed);
        // Each commit is listed after the commits it reaches.
        let versions = listed
            .into_iter()
            .map(|(index, parents)| {
                let commit = &self.commits[index];
                FileVersion::new(
                    commit.name.clone(),
                    parents
                        .iter()
                        .map(|parent| self.commits[*parent].name.clone())
                        .collect(),
                    commit.author.clone(),
                    commit.committed_at,
                    self.files[index].get(path).cloned(),
                )
            })
            .collect();
        Box::pin(std::future::ready(Ok(Arc::new(versions))))
    }

    fn texts<'a>(
        &'a self,
        path: &'a str,
        commits: &'a [String],
    ) -> BoxFuture<'a, anyhow::Result<Vec<Option<String>>>> {
        let texts = commits
            .iter()
            .map(|commit| {
                self.files[Self::at(&self.commits, commit)]
                    .get(path)
                    .cloned()
            })
            .collect();
        Box::pin(std::future::ready(Ok(texts)))
    }
}

struct WrongMerges;

impl Harness for WrongMerges {
    fn adapter(&self, commits: &[Commit]) -> Adapter {
        let ids = commits
            .iter()
            .map(|commit| (commit.name.clone(), commit.name.clone()))
            .collect();
        Adapter::new(Arc::new(FirstParentOnly::new(commits)), ids)
    }
}

#[tokio::test]
#[should_panic(
    expected = ".provenance/state/records.jsonl: the last version is not the newest commit that changed the file"
)]
async fn the_contract_refuses_an_adapter_that_compares_a_merge_with_its_first_parent_only() {
    contract::versions_follow_the_history_of_head(&WrongMerges).await;
}

async fn history(repository: &Repository, id: StableId) -> anyhow::Result<Vec<RecordVersion>> {
    let query = ReviewHistoryQuery {
        record_kind: NodeType::Requirement,
        record_id: id,
        limit: 10,
        cursor: None,
    };
    Ok(read_history(repository, ReadPolicy::default(), query)
        .await?
        .result
        .entries)
}

fn ids(entries: &[RecordVersion]) -> Vec<&str> {
    entries.iter().map(|entry| entry.id.as_str()).collect()
}

/// Implementation aid with no Rule binding: a repository that is given file
/// versions reads its record versions, and the Discussion file at each
/// version, through them and not through Git. The fixture folder has no Git
/// history, so a read through Git lists only the working copy and finds no
/// outcome.
#[tokio::test]
async fn a_repository_reads_record_history_through_its_file_versions() {
    let (temp, store) = discussion_support::fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let repository = discussion_support::repository(root);
    let scope = discussion_support::scope();
    let shard = shards::path_for(repository.layout(), &scope, NodeType::Requirement);
    let discussions = shards::discussions_path(repository.layout(), &scope);
    let earlier = std::fs::read_to_string(&shard).unwrap();
    let discussion = discussion_support::start(&store, "Use text A.");
    let origin = DiscussionOrigin {
        discussion_id: discussion.discussion_id.clone(),
        thread_id: discussion.thread_id.clone(),
        message_id: discussion.root_message_id.clone(),
    };
    let save = discussion_support::save(&store, json!({"description":"Text A."}));
    store
        .save_requirement_from_discussion(save, &origin)
        .unwrap();
    let relative = |path: &Utf8Path| {
        path.strip_prefix(repository.working_tree())
            .unwrap()
            .as_str()
            .replace('\\', "/")
    };
    let commits = [
        Commit::new("first", &[], "ann", 1_700_000_100).with_file(&relative(&shard), &earlier),
        Commit::new("second", &["first"], "bea", 1_700_000_200)
            .with_file(&relative(&shard), &std::fs::read_to_string(&shard).unwrap())
            .with_file(
                &relative(&discussions),
                &std::fs::read_to_string(&discussions).unwrap(),
            ),
    ];
    let repository = repository.with_file_versions(Arc::new(MemoryFileVersions::new(&commits)));
    let entries = history(&repository, discussion_support::id())
        .await
        .unwrap();
    assert_eq!(ids(&entries), ["first", "second"]);
    assert_eq!(entries[1].author.as_deref(), Some("bea"));
    assert_eq!(entries[1].committed_at, Some(1_700_000_200));
    assert_eq!(entries[1].changed_fields, ["description"]);
    assert_eq!(entries[0].origin, None);
    assert_eq!(entries[1].origin, Some(origin));
}

/// Implementation aid with no Rule binding: a repository over storage that
/// is not a folder shows its working version from that storage. Its working
/// tree is an empty folder, so a read of the record file from disk finds
/// none.
#[tokio::test]
async fn a_repository_over_other_storage_shows_its_working_version_from_storage() {
    let memory = memory::memory_repository();
    let repository = memory
        .repository
        .with_file_versions(Arc::new(MemoryFileVersions::new(&[])));
    let entries = history(&repository, memory::id("req_expiry"))
        .await
        .unwrap();
    assert_eq!(ids(&entries), ["working"]);
    assert_eq!(entries[0].commit, None);
    memory.assert_empty();
}

/// Implementation aid with no Rule binding: on an opened repository, a
/// record file of the working copy that is not UTF-8 refuses the history
/// read with the text of the storage view, which names the file.
#[tokio::test]
async fn a_working_record_file_that_is_not_utf8_refuses_the_history_read() {
    let (temp, _store) = discussion_support::fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let repository = discussion_support::repository(root);
    let scope = discussion_support::scope();
    let shard = shards::path_for(repository.layout(), &scope, NodeType::Requirement);
    std::fs::write(&shard, b"\xff\xfe\n").unwrap();
    let error = history(&repository, discussion_support::id())
        .await
        .unwrap_err();
    assert_eq!(
        format!("{error:#}"),
        format!("read {shard}: the file is not UTF-8: invalid data")
    );
}
