//! The file versions contract on the git adapter and on the memory adapter,
//! and record history through the file versions that a repository is given.
//!
//! No Rule: the plan proposes `rule_record_history_adapters_agree` for the
//! owner's review. Until it is approved, these tests hold the contract: the
//! git adapter shows that the expected answers are Git's, and the memory
//! adapter shows that an adapter with no Git gives the same answers.

use crate::discussion_support;
use crate::memory;
#[path = "history_versions_contract/edges.rs"]
mod edges;
#[path = "history_versions_contract/git_harness.rs"]
mod git_harness;
#[path = "history_versions_contract/subdirectory.rs"]
mod subdirectory;
#[path = "history_versions_contract/wrong.rs"]
mod wrong;

use camino::{Utf8Path, Utf8PathBuf};
use git_harness::git_harness;
use provenance_core::review::{RecordVersion, ReviewHistoryQuery};
use provenance_core::threads::DiscussionOrigin;
use provenance_core::{NodeType, StableId};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use provenance_store::read_database::file::FileReadDatabase;
use provenance_store::repository::{Repository, RepositoryIdentity};
use provenance_store::review::history::contract::{
    self, Adapter, Commit, Harness, MemoryFileVersions,
};
use provenance_store::review::read_history;
use provenance_store::settings::Settings;
use provenance_store::shards;
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use serde_json::json;
use std::sync::Arc;
use wrong::Wrong;

pub struct Memory;

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
async fn git_text_gives_the_file_at_a_commit() {
    contract::text_gives_the_file_at_a_commit(&git_harness()).await;
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
async fn memory_text_gives_the_file_at_a_commit() {
    contract::text_gives_the_file_at_a_commit(&Memory).await;
}

#[tokio::test]
async fn memory_a_file_that_no_commit_holds_has_no_versions() {
    contract::a_file_that_no_commit_holds_has_no_versions(&Memory).await;
}

#[tokio::test]
#[should_panic(
    expected = ".provenance/state/records.jsonl: the last version is not the first commit that changed the file on the walk from HEAD"
)]
async fn the_contract_refuses_an_adapter_that_compares_a_merge_with_its_first_parent_only() {
    contract::versions_follow_the_history_of_head(&Wrong(wrong::first_parent_only)).await;
}

#[tokio::test]
#[should_panic(
    expected = ".provenance/state/merged.jsonl: the listed commits, parents or texts are not the expected ones"
)]
async fn the_contract_refuses_an_adapter_that_goes_through_the_last_parent_with_the_same_file() {
    contract::versions_follow_the_history_of_head(&Wrong(wrong::last_same_parent)).await;
}

#[tokio::test]
#[should_panic(
    expected = ".provenance/state/moved_to.jsonl: the listed commits, parents or texts are not the expected ones"
)]
async fn the_contract_refuses_an_adapter_that_follows_a_moved_file() {
    contract::versions_follow_the_history_of_head(&Wrong(wrong::follows_moves)).await;
}

#[tokio::test]
#[should_panic(
    expected = ".provenance/state/records.jsonl: a version does not carry the commit time of its commit"
)]
async fn the_contract_refuses_an_adapter_that_gives_the_author_time() {
    contract::versions_follow_the_history_of_head(&Wrong(wrong::author_time)).await;
}

#[tokio::test]
#[should_panic(
    expected = ".provenance/state/records.jsonl: a version does not carry the author name of its commit"
)]
async fn the_contract_refuses_an_adapter_that_gives_the_committer_name() {
    contract::versions_follow_the_history_of_head(&Wrong(wrong::committer_name)).await;
}

async fn history_with(
    repository: &Repository,
    id: StableId,
    policy: ReadPolicy,
) -> anyhow::Result<Vec<RecordVersion>> {
    let query = ReviewHistoryQuery {
        record_kind: NodeType::Requirement,
        record_id: id,
        limit: 10,
        cursor: None,
    };
    Ok(read_history(repository, policy, query)
        .await?
        .result
        .entries)
}

async fn history(repository: &Repository, id: StableId) -> anyhow::Result<Vec<RecordVersion>> {
    history_with(repository, id, ReadPolicy::default()).await
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
        message_id: discussion.root_message_id,
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
/// tree is an empty folder and its read database is in another folder, so a
/// read of the record file from disk finds none.
#[tokio::test]
async fn a_repository_over_other_storage_shows_its_working_version_from_storage() {
    let root = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(cache.path()).unwrap());
    std::fs::create_dir_all(layout.cache_dir()).unwrap();
    let working_tree =
        Utf8PathBuf::from_path_buf(dunce::canonicalize(root.path()).unwrap()).unwrap();
    let identity = RepositoryIdentity::new(working_tree, memory::scope(), Settings::default())
        .with_file_versions(Arc::new(MemoryFileVersions::new(&[])));
    let repository = Repository::with_storage(
        identity,
        Arc::new(MemoryStateStorage::new(
            memory::initial_state().into_entries(),
        )),
        Arc::new(MemorySectionLock::default()),
        Some(Arc::new(FileReadDatabase::new(layout))),
    )
    .unwrap();
    let entries = history(&repository, memory::id("req_expiry"))
        .await
        .unwrap();
    assert_eq!(ids(&entries), ["working"]);
    assert_eq!(entries[0].commit, None);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

/// Implementation aid with no Rule binding: on an opened repository, a
/// record file of the working copy that is not UTF-8 refuses a history read
/// that does not catch up first with the text of the storage view, which
/// names the file. A read that catches up refuses at the catch-up.
#[tokio::test]
async fn a_working_record_file_that_is_not_utf8_refuses_the_history_read() {
    let (temp, _store) = discussion_support::fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let repository = discussion_support::repository(root);
    let scope = discussion_support::scope();
    let shard = shards::path_for(repository.layout(), &scope, NodeType::Requirement);
    history(&repository, discussion_support::id())
        .await
        .unwrap();
    std::fs::write(&shard, b"\xff\xfe\n").unwrap();
    let annotate = ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly);
    let error = history_with(&repository, discussion_support::id(), annotate)
        .await
        .unwrap_err();
    assert_eq!(
        format!("{error:#}"),
        format!("read {shard}: the file is not UTF-8: invalid data")
    );
}
