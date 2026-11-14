//! A repository over the memory adapter, rooted at an empty directory.
//!
//! The state starts with the manifest, which declares the default scope and
//! the scope `imported`, one active Requirement and one active Rule. A test
//! can add more state entries. The adapter gives each write set to a
//! persistence that keeps it. The start of a verification run of the Rule
//! has its own helpers.

#![allow(dead_code)]

use provenance_core::{Manifest, RepoPathPrefix, Scope, ScopeId, StableId};
use provenance_store::repository::{Clock, Repository, RepositoryIdentity, SystemClock};
use provenance_store::settings::Settings;
use provenance_store::state_store::BeginVerificationInput;
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use provenance_store::storage::{
    PublishFailure, StateKey, StatePersistence, StateStorage, StateView, WriteSet,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// Keeps each write set that the adapter gives it.
#[derive(Debug, Default)]
pub struct Recorded(pub Mutex<Vec<WriteSet>>);

impl StatePersistence for Recorded {
    fn persist(&self, writes: &WriteSet) -> Result<(), PublishFailure> {
        self.0.lock().unwrap().push(writes.clone());
        Ok(())
    }
}

/// How a [`Refusing`] persistence fails a write set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    /// No key landed.
    Refused,
    /// Landing began, and the outcome is not known.
    Started,
}

/// Keeps each write set that the adapter gives it. While a failure is set,
/// it fails each set that puts a key that `puts` accepts.
#[derive(Debug)]
pub struct Refusing {
    puts: fn(&str) -> bool,
    failure: Mutex<Option<Failure>>,
    received: Mutex<Vec<WriteSet>>,
}

impl Refusing {
    pub fn new(puts: fn(&str) -> bool) -> Self {
        Self {
            puts,
            failure: Mutex::new(None),
            received: Mutex::new(Vec::new()),
        }
    }

    /// Fails the later sets with `failure`, or with no failure.
    pub fn fail(&self, failure: Option<Failure>) {
        *self.failure.lock().unwrap() = failure;
    }

    /// The sets that the persistence received, from the first.
    pub fn received(&self) -> Vec<WriteSet> {
        self.received.lock().unwrap().clone()
    }
}

impl StatePersistence for Refusing {
    fn persist(&self, writes: &WriteSet) -> Result<(), PublishFailure> {
        self.received.lock().unwrap().push(writes.clone());
        if !writes.puts().keys().any(|key| (self.puts)(key.as_str())) {
            return Ok(());
        }
        let failure = *self.failure.lock().unwrap();
        match failure {
            None => Ok(()),
            Some(Failure::Refused) => Err(PublishFailure::Refused(anyhow::anyhow!(
                "the persistence refused the set"
            ))),
            Some(Failure::Started) => Err(PublishFailure::Started(anyhow::anyhow!(
                "the persistence stopped while it kept the set"
            ))),
        }
    }
}

/// A clock that always reads one time.
#[derive(Debug)]
pub struct FixedClock(pub time::OffsetDateTime);

impl FixedClock {
    /// A clock that reads `millis` milliseconds after the Unix epoch.
    pub fn at_millis(millis: i64) -> Self {
        let nanos = i128::from(millis) * 1_000_000;
        Self(time::OffsetDateTime::from_unix_timestamp_nanos(nanos).unwrap())
    }
}

impl Clock for FixedClock {
    fn now(&self) -> time::OffsetDateTime {
        self.0
    }
}

pub fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

pub fn imported() -> ScopeId {
    ScopeId::new("imported").unwrap()
}

pub fn id(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

pub fn key(value: &str) -> StateKey {
    StateKey::new(value).unwrap()
}

/// The JSONL bytes of `rows`.
pub fn shard(rows: &[serde_json::Value]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for row in rows {
        bytes.extend(serde_json::to_vec(row).unwrap());
        bytes.push(b'\n');
    }
    bytes
}

/// The manifest, one active Requirement and one active Rule.
pub fn initial_state() -> StateView {
    StateView::new(initial_entries())
}

fn initial_entries() -> BTreeMap<StateKey, Vec<u8>> {
    let mut manifest = Manifest::default_with_scope(scope(), RepoPathPrefix::new("."));
    manifest.scopes.push(Scope {
        id: imported(),
        path_prefix: RepoPathPrefix::new("imported"),
    });
    let requirement = serde_json::json!({
        "scope_id": "default",
        "id": "req_expiry",
        "statement": "Share links expire",
        "status": "active",
    });
    let rule = serde_json::json!({
        "scope_id": "default",
        "id": "rule_expiry",
        "statement": "Share links expire",
        "status": "active",
        "severity": "medium",
        "requirement_ids": ["req_expiry"],
    });
    BTreeMap::from([
        (key("manifest.json"), serde_json::to_vec(&manifest).unwrap()),
        (
            key("scopes/default/requirements/req.jsonl"),
            shard(&[requirement]),
        ),
        (key("scopes/default/rules/rule.jsonl"), shard(&[rule])),
    ])
}

pub struct MemoryRepository {
    pub root: tempfile::TempDir,
    pub storage: Arc<MemoryStateStorage>,
    pub recorded: Arc<Recorded>,
    pub repository: Repository,
    initial: StateView,
}

impl MemoryRepository {
    /// The working tree has no entry.
    pub fn assert_empty(&self) {
        let entries: Vec<_> = std::fs::read_dir(self.root.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert!(
            entries.is_empty(),
            "the repository wrote files: {entries:?}"
        );
    }

    /// The state that the recorded write sets make from the initial state.
    pub fn replayed(&self) -> StateView {
        let mut entries = self.initial.clone().into_entries();
        for writes in self.recorded.0.lock().unwrap().iter() {
            writes.apply_to(&mut entries);
        }
        MemoryStateStorage::new(entries).read_view().unwrap()
    }
}

pub fn memory_repository() -> MemoryRepository {
    memory_repository_with([])
}

/// A memory repository whose state also holds `extra`.
pub fn memory_repository_with(
    extra: impl IntoIterator<Item = (StateKey, Vec<u8>)>,
) -> MemoryRepository {
    let mut entries = initial_entries();
    entries.extend(extra);
    let initial = StateView::new(entries);
    let root = tempfile::tempdir().unwrap();
    let recorded = Arc::new(Recorded::default());
    let (storage, repository) =
        memory_repository_over(root.path(), initial.clone(), recorded.clone());
    MemoryRepository {
        root,
        storage,
        recorded,
        repository,
        initial,
    }
}

/// A repository over the memory adapter, rooted at `root`, whose state
/// starts as `initial`. The adapter gives each write set to `persistence`.
pub fn memory_repository_over(
    root: &std::path::Path,
    initial: StateView,
    persistence: Arc<dyn StatePersistence>,
) -> (Arc<MemoryStateStorage>, Repository) {
    memory_repository_at(root, initial, persistence, Arc::new(SystemClock))
}

/// Like [`memory_repository_over`], but the writes read the time from `clock`.
pub fn memory_repository_at(
    root: &std::path::Path,
    initial: StateView,
    persistence: Arc<dyn StatePersistence>,
    clock: Arc<dyn Clock>,
) -> (Arc<MemoryStateStorage>, Repository) {
    let storage = Arc::new(MemoryStateStorage::loaded(initial, persistence));
    let working_tree =
        camino::Utf8PathBuf::from_path_buf(dunce::canonicalize(root).unwrap()).unwrap();
    let identity =
        RepositoryIdentity::new(working_tree, scope(), Settings::default()).with_clock(clock);
    let repository = Repository::with_storage(
        identity,
        storage.clone(),
        Arc::new(MemorySectionLock::default()),
        None,
    )
    .unwrap();
    (storage, repository)
}

/// The key of the reviews shard of the default scope.
pub const REVIEWS: &str = "scopes/default/requirements/review.jsonl";

/// An open review of `rule_expiry`, raised when the statement of
/// `req_expiry` changed.
pub fn open_review() -> serde_json::Value {
    serde_json::json!({
        "scope_id": "default", "id": "review_expiry", "rule_id": "rule_expiry",
        "requirement_id": "req_expiry", "field": "statement",
        "before": "Links expire", "after": "Share links expire", "changed_at": 1
    })
}

/// A start of a verification run of `rule_expiry` whose evidence is the
/// function `expiry` in `evidence.rs`.
pub fn begin_input() -> BeginVerificationInput {
    serde_json::from_value(serde_json::json!({
        "rule": "rule_expiry", "key": "expiry", "method": "examples",
        "declared_by": "suite", "file": "evidence.rs", "symbol": "expiry"
    }))
    .unwrap()
}

/// A memory repository at `root` whose state also holds `extra` and the
/// rows `reviews` as the reviews shard of the default scope, and whose
/// working tree holds `evidence.rs`. With no rows the state has no reviews
/// shard.
pub fn verification_repository(
    root: &std::path::Path,
    reviews: &[serde_json::Value],
    extra: impl IntoIterator<Item = (StateKey, Vec<u8>)>,
    persistence: Arc<dyn StatePersistence>,
    clock: Arc<dyn Clock>,
) -> (Arc<MemoryStateStorage>, Repository) {
    std::fs::write(root.join("evidence.rs"), "fn expiry() {}\n").unwrap();
    let mut entries = initial_entries();
    if !reviews.is_empty() {
        entries.insert(key(REVIEWS), shard(reviews));
    }
    entries.extend(extra);
    memory_repository_at(root, StateView::new(entries), persistence, clock)
}
