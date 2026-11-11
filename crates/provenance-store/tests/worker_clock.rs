//! The store reads the time through the time crate, which reaches the clock
//! through JavaScript on wasm32. The standard library clock panics in a
//! Worker, so only the local adapter, which never runs there, may name it.

#[path = "support/memory.rs"]
mod memory;

use memory::{id, initial_state, scope};
use provenance_core::{MessageRole, NodeType, RequirementStatus, ThreadParent};
use provenance_macros::verifies;
use provenance_store::repository::{
    Clock, IdSource, Repository, RepositoryHistory, RepositoryIdentity,
};
use provenance_store::settings::Settings;
use provenance_store::state_store::{CreateRequirementInput, PostMessageInput};
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const STD_CLOCK: &str = "SystemTime";

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            pending.extend(
                fs::read_dir(&path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    found.sort();
    found
}

/// The local adapter: `storage/local.rs` and everything under `storage/local/`.
fn is_local_adapter(path: &Path, root: &Path) -> bool {
    let relative = path.strip_prefix(root).unwrap();
    let local = Path::new("storage").join("local");
    relative.starts_with(&local) || relative == local.with_extension("rs")
}

fn names_std_clock(source: &str) -> bool {
    source
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|word| word == STD_CLOCK)
}

#[test]
#[verifies("rule_store_time_works_in_a_worker", examples)]
fn store_code_outside_the_local_adapter_names_no_std_clock() {
    let root = src();
    let findings: Vec<String> = rust_files(&root)
        .into_iter()
        .filter(|path| !is_local_adapter(path, &root))
        .filter(|path| names_std_clock(&fs::read_to_string(path).unwrap()))
        .map(|path| path.strip_prefix(&root).unwrap().display().to_string())
        .collect();
    assert!(
        findings.is_empty(),
        "these files name {STD_CLOCK}; read the time with time::OffsetDateTime::now_utc: {findings:?}"
    );
}

#[test]
fn the_scan_finds_the_std_clock_and_skips_the_local_adapter() {
    assert!(names_std_clock("let now = std::time::SystemTime::now();"));
    assert!(!names_std_clock("let now = MySystemTimeLike::now();"));
    let root = src();
    assert!(is_local_adapter(
        &root.join("storage/local/whole_set.rs"),
        &root
    ));
    assert!(is_local_adapter(&root.join("storage/local.rs"), &root));
    assert!(!is_local_adapter(&root.join("storage/memory.rs"), &root));
}

/// A clock that always reads one time.
#[derive(Debug)]
struct FixedClock(time::OffsetDateTime);

impl Clock for FixedClock {
    fn now(&self) -> time::OffsetDateTime {
        self.0
    }
}

/// IDs that count up from one.
#[derive(Debug, Default)]
struct CountingIds(std::sync::atomic::AtomicU64);

impl IdSource for CountingIds {
    fn new_uuid(&self) -> uuid::Uuid {
        let next = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        uuid::Uuid::from_u128(u128::from(next))
    }
}

/// A history whose head is one fixed commit, so writes stamp records.
#[derive(Debug)]
struct OneCommit;

impl RepositoryHistory for OneCommit {
    fn head(&self) -> anyhow::Result<Option<String>> {
        Ok(Some("a".repeat(40)))
    }
    fn complete_head(&self) -> anyhow::Result<Option<String>> {
        self.head()
    }
    fn commit_exists(&self, _commit: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
    fn is_ancestor(&self, _commit: &str, _head: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
}

// No Rule binding: this test is an implementation aid for the simulation,
// and the owner declined a Rule for the clock and ID source (2026-11-10).
#[test]
fn record_stamps_and_message_ids_come_from_the_identity() {
    let at = time::OffsetDateTime::from_unix_timestamp(981_173_106).unwrap();
    let identity = RepositoryIdentity::new(
        camino::Utf8PathBuf::from("/simulated"),
        scope(),
        Settings::default(),
    )
    .with_history(Arc::new(OneCommit))
    .with_clock(Arc::new(FixedClock(at)))
    .with_ids(Arc::new(CountingIds::default()));
    let repository = Repository::with_storage(
        identity,
        Arc::new(MemoryStateStorage::new(initial_state().into_entries())),
        Arc::new(MemorySectionLock::default()),
    )
    .unwrap();
    let store = repository.store();

    let requirement = store
        .create_requirement(CreateRequirementInput {
            scope_id: scope(),
            id: id("req_owner"),
            statement: "Share links have an owner".into(),
            description: None,
            status: RequirementStatus::Active,
            refines: Vec::new(),
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    assert_eq!(requirement.created.unwrap().at, "2001-02-03T04:05:06Z");

    let posted = store
        .post_thread_message(PostMessageInput {
            scope_id: scope(),
            parent: ThreadParent {
                node_type: NodeType::Requirement,
                node_id: id("req_expiry"),
            },
            role: MessageRole::User,
            body: "Who owns a link?".into(),
        })
        .unwrap();
    // The counting source gives every new ID of the write, the Thread's too.
    assert!(
        posted
            .message
            .id
            .as_str()
            .starts_with("msg_00000000-0000-0000-0000-00000000000"),
        "the message ID comes from the counting source: {}",
        posted.message.id.as_str()
    );
    assert_eq!(posted.message.created_at, at.unix_timestamp() * 1000);
}
