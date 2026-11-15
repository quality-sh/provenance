//! The history operations on one Git history: a merge, a Discussion
//! outcome committed before its record, a deleted record, a renamed record
//! file, a record file that was not UTF-8 at one commit, and an entry id
//! that names no commit.
//!
//! `CASES` (`history_cases.rs`) holds one row for each history operation of
//! each reviewed kind, as the contract routes serve them. The answers,
//! without their stamps and with each commit id as its name in the graph,
//! equal the checked file over a folder repository with real Git history.
//!
//! Over memory storage, with the history contract's memory adapter of the
//! same commits and a read database in memory, each operation runs through
//! the in-process port and gives the same answers, with and without
//! `host`. A repository that was given no file versions, at a working tree
//! that does not exist, shows the saved version only, with and without
//! `host`.
//!
//! Implementation aid with no Rule binding: these tests pin the answers of
//! the history operations so that a change of the path they read through
//! cannot change them.

use crate::history_cases::{run, Reads, CASES};
use crate::history_fixture::{history, History, Sources};
use crate::memory::{FixedClock, Recorded};
use crate::read_in;
use crate::sqlite_database::MemorySqlite;
use provenance_store::cache::catch_up_in_publish;
use provenance_store::operations::read_operations::READ_OPERATIONS;
use provenance_store::read_database::contract::Other;
use provenance_store::repository::{IdSource, Repository, RepositoryIdentity};
use provenance_store::review::history::contract::MemoryFileVersions;
use provenance_store::review::history::FileVersions;
use provenance_store::settings::Settings;
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use provenance_store::storage::StateView;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// The checked answers of the cases.
const PINNED: &str = include_str!("../fixtures/history_operations.json");
/// The checked answers of the cases over a repository with no file versions.
const WITHOUT_VERSIONS: &str = include_str!("../fixtures/history_without_versions.json");

/// Ids made from a counter, so each run makes the same ids.
#[derive(Debug, Default)]
struct Counted(AtomicU64);

impl IdSource for Counted {
    fn new_uuid(&self) -> uuid::Uuid {
        uuid::Uuid::from_u128(u128::from(self.0.fetch_add(1, Ordering::SeqCst) + 1))
    }
}

/// The history, with fixed times and ids.
pub fn fixture() -> History {
    history(&Sources {
        clock: Arc::new(FixedClock::at_millis(1_700_000_000_000)),
        ids: Arc::new(Counted::default()),
    })
}

/// Fails with each line of `observed` that differs from the checked text
/// `pinned`, and with the whole observed text.
pub fn assert_pinned(pinned: &str, observed: &str) {
    let expected = pinned.trim_end();
    if observed == expected {
        return;
    }
    let (observed_lines, expected_lines): (Vec<_>, Vec<_>) =
        (observed.lines().collect(), expected.lines().collect());
    let mut diff = Vec::new();
    for index in 0..observed_lines.len().max(expected_lines.len()) {
        let (left, right) = (expected_lines.get(index), observed_lines.get(index));
        if left != right {
            diff.push(format!("line {}:", index + 1));
            diff.extend(left.map(|line| format!("- {line}")));
            diff.extend(right.map(|line| format!("+ {line}")));
        }
        if diff.len() > 60 {
            break;
        }
    }
    panic!(
        "the answers differ from the checked file:\n{}\n=== OBSERVED\n{observed}\n=== END",
        diff.join("\n")
    );
}

/// Runs each operation through the in-process port of a repository over
/// memory storage, one for each scope: a binding serves one scope.
pub struct MemoryReads {
    pub default: Repository,
    pub broken: Repository,
}

impl Reads for MemoryReads {
    fn read<'a>(
        &'a self,
        scope: &'a str,
        operation: &'static str,
        request: Value,
    ) -> Pin<Box<dyn Future<Output = Value> + Send + 'a>> {
        let repository = if scope == "broken" {
            &self.broken
        } else {
            &self.default
        };
        let context = json!({"repository": "memory", "scope": scope});
        Box::pin(read_in(repository, context, operation, request))
    }
}

/// A repository of `scope` over memory storage that holds the working copy
/// of `history`, at a working tree that does not exist, with the file
/// versions `versions` when there are some, and a read database in memory,
/// caught up.
pub async fn memory_repository(
    history: &History,
    scope: &str,
    versions: Option<Arc<dyn FileVersions>>,
) -> Repository {
    let storage = Arc::new(MemoryStateStorage::loaded(
        StateView::new(history.state()),
        Arc::new(Recorded::default()),
    ));
    let missing = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(missing.path().join("project")).unwrap();
    let identity = RepositoryIdentity::new(
        root,
        provenance_core::ScopeId::new(scope).unwrap(),
        Settings::default(),
    );
    let repository = Repository::with_storage(
        identity,
        storage,
        Arc::new(MemorySectionLock::refusing()),
        Some(Arc::new(Other::new(MemorySqlite::new().await))),
    )
    .unwrap();
    let repository = match versions {
        Some(versions) => repository.with_file_versions(versions),
        None => repository,
    };
    catch_up_in_publish(&repository).await.unwrap();
    repository
}

#[tokio::test]
async fn the_history_operations_over_memory_give_the_answers_of_the_folder() {
    let history = fixture();
    let versions = || Some(Arc::new(MemoryFileVersions::new(&history.commits)) as _);
    let reads = MemoryReads {
        default: memory_repository(&history, "default", versions()).await,
        broken: memory_repository(&history, "broken", versions()).await,
    };
    let answers = run(&reads, &CASES).await;
    assert_pinned(PINNED, &serde_json::to_string_pretty(&answers).unwrap());
}

/// A repository that was given no file versions, at a working tree that is
/// not a Git work tree, has no committed history: each record that its
/// working copy holds lists the saved version only. The build with `host`
/// reads Git there and finds no history, and the build without `host` has
/// no committed history, so both give the same answers.
#[tokio::test]
async fn a_repository_with_no_file_versions_shows_the_saved_version_only() {
    let history = fixture();
    let reads = MemoryReads {
        default: memory_repository(&history, "default", None).await,
        broken: memory_repository(&history, "broken", None).await,
    };
    let answers = run(&reads, &CASES).await;
    assert_pinned(
        WITHOUT_VERSIONS,
        &serde_json::to_string_pretty(&answers).unwrap(),
    );
}

/// Implementation aid: each operation of the port is a read of the review
/// page or a row of `CASES`, so a new operation of the port fails here
/// until a test covers it.
#[test]
fn each_operation_of_the_port_has_a_case() {
    let history: BTreeSet<_> = CASES.iter().map(|case| case.operation).collect();
    let reads: BTreeSet<_> = crate::answers::reads()
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    let covered: BTreeSet<_> = history.union(&reads).copied().collect();
    assert_eq!(covered, READ_OPERATIONS.iter().copied().collect());
    assert!(history.is_disjoint(&reads));
}

#[cfg(feature = "host")]
mod folder {
    use super::{assert_pinned, fixture, PINNED};
    use crate::history_cases::{run, Reads, CASES};
    use crate::history_folder::folder;
    use provenance_core::protocol::repository::RepositoryContext;
    use provenance_store::operations::catalog::{
        frame_failure, Direct, Invoke as _, Operation, RequestedContext, ReviewEvidence,
        ReviewHistory, ReviewHistoryEntry,
    };
    use provenance_store::operations::LocalRepository;
    use provenance_store::repository::Repository;
    use serde_json::{json, Value};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;

    /// Runs each operation on a folder repository as the contract dispatch
    /// runs it on the host, with no port, with one binding for each scope.
    struct FolderReads {
        default: Arc<LocalRepository>,
        broken: Arc<LocalRepository>,
    }

    async fn direct<O: Operation>(
        binding: Arc<LocalRepository>,
        scope: &str,
        request: Value,
    ) -> Value {
        let context: RepositoryContext =
            serde_json::from_value(json!({"repository": "memory", "scope": scope})).unwrap();
        let request: O::Request = serde_json::from_value(request).unwrap();
        match Direct(binding)
            .invoke::<O>(Some(RequestedContext::Scoped(context)), request)
            .await
        {
            Ok(success) => serde_json::to_value(success).unwrap(),
            Err(failure) => {
                let failure = frame_failure::<O>(failure);
                json!({"status": failure.status_code(), "error": failure.error})
            }
        }
    }

    impl Reads for FolderReads {
        fn read<'a>(
            &'a self,
            scope: &'a str,
            operation: &'static str,
            request: Value,
        ) -> Pin<Box<dyn Future<Output = Value> + Send + 'a>> {
            let binding = Arc::clone(if scope == "broken" {
                &self.broken
            } else {
                &self.default
            });
            match operation {
                "review-history" => Box::pin(direct::<ReviewHistory>(binding, scope, request)),
                "review-history-entry" => {
                    Box::pin(direct::<ReviewHistoryEntry>(binding, scope, request))
                }
                "review-evidence" => Box::pin(direct::<ReviewEvidence>(binding, scope, request)),
                other => panic!("{other} is not a history operation"),
            }
        }
    }

    #[tokio::test]
    async fn the_history_operations_over_a_folder_give_the_checked_answers() {
        let folder = folder(&fixture());
        let bound = |scope| {
            let repository = Repository::open(folder.root(), scope).unwrap();
            Arc::new(LocalRepository::bind(repository, "memory").unwrap())
        };
        let reads = FolderReads {
            default: bound("default"),
            broken: bound("broken"),
        };
        let answers = run(&reads, &CASES).await;
        let text = serde_json::to_string_pretty(&answers).unwrap();
        assert_pinned(PINNED, &folder.named(&text));
    }
}
