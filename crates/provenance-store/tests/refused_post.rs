//! A message post over the memory adapter, where the persistence keeps each
//! write set that it receives and can refuse each set that changes a Message
//! shard.

#[path = "support/memory.rs"]
mod memory;

use memory::{id, initial_state, memory_repository_over, scope};
use provenance_core::{Message, MessageRole, NodeType, Thread, ThreadParent};
use provenance_macros::verifies;
use provenance_store::repository::Repository;
use provenance_store::state_store::PostMessageInput;
use provenance_store::storage::memory::MemoryStateStorage;
use provenance_store::storage::{Atomicity, PublishFailure, StatePersistence, WriteSet};
use provenance_store::storage::{StateKey, StateStorage};
use provenance_store::write_error::{WriteError, WriteFailure};
use std::sync::{Arc, Mutex};

const THREAD_DIRECTORY: &str = "scopes/default/threads/";
const THREADS: &str = "scopes/default/threads/threads.jsonl";

/// Keeps each set that it receives. Refuses each set that puts a Message
/// shard when `refuse` is set.
#[derive(Debug, Default)]
struct Sets {
    refuse: bool,
    received: Mutex<Vec<WriteSet>>,
}

impl Sets {
    /// The sets that the persistence received, from the first.
    fn received(&self) -> Vec<WriteSet> {
        self.received.lock().unwrap().clone()
    }
}

impl StatePersistence for Sets {
    fn persist(&self, writes: &WriteSet) -> Result<(), PublishFailure> {
        self.received.lock().unwrap().push(writes.clone());
        let keys = writes.puts().keys();
        if self.refuse && keys.map(StateKey::as_str).any(is_message_shard) {
            return Err(PublishFailure::Refused(anyhow::anyhow!(
                "the persistence refused the Message shard"
            )));
        }
        Ok(())
    }
}

fn is_message_shard(key: &str) -> bool {
    key.starts_with(THREAD_DIRECTORY) && key != THREADS
}

/// A memory repository over a persistence that refuses each set that puts
/// a Message shard when `refuse` is set.
struct Posting {
    _root: tempfile::TempDir,
    persistence: Arc<Sets>,
    storage: Arc<MemoryStateStorage>,
    repository: Repository,
}

fn posting(refuse: bool) -> Posting {
    let root = tempfile::tempdir().unwrap();
    let persistence = Arc::new(Sets {
        refuse,
        ..Sets::default()
    });
    let (storage, repository) =
        memory_repository_over(root.path(), initial_state(), persistence.clone());
    Posting {
        _root: root,
        persistence,
        storage,
        repository,
    }
}

fn post() -> PostMessageInput {
    PostMessageInput {
        scope_id: scope(),
        parent: ThreadParent {
            node_type: NodeType::Requirement,
            node_id: id("req_expiry"),
        },
        role: MessageRole::User,
        body: "Who owns a link?".into(),
    }
}

/// The keys of the Thread shard and the Message shards in the state.
fn thread_keys(storage: &MemoryStateStorage) -> Vec<StateKey> {
    let view = storage.read_view().unwrap();
    let is_thread = |key: &&StateKey| key.as_str().starts_with(THREAD_DIRECTORY);
    view.entries().keys().filter(is_thread).cloned().collect()
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
fn a_refused_post_lands_no_thread_and_no_message() {
    let posting = posting(true);
    let store = posting.repository.store();

    let error = store.post_thread_message(post()).unwrap_err();

    assert!(matches!(
        WriteError::from(error).safe(),
        WriteFailure::WriteFailed { .. }
    ));
    let attempts = posting.persistence.received();
    assert_eq!(attempts.len(), 1, "one attempt: {attempts:?}");
    assert_eq!(thread_keys(&posting.storage), [] as [StateKey; 0]);
    assert_eq!(store.list_threads(&scope()).unwrap(), [] as [Thread; 0]);
    assert_eq!(store.list_messages(&scope()).unwrap(), [] as [Message; 0]);
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
fn a_post_publishes_the_thread_and_the_message_as_one_whole_set() {
    let posting = posting(false);
    let store = posting.repository.store();

    let posted = store.post_thread_message(post()).unwrap();

    let sets = posting.persistence.received();
    assert_eq!(sets.len(), 1, "one set: {sets:?}");
    assert_eq!(sets[0].atomicity(), Atomicity::WholeSet);
    let keys: Vec<_> = sets[0].puts().keys().cloned().collect();
    assert_eq!(keys, thread_keys(&posting.storage));
    assert_eq!(keys.len(), 2, "{keys:?}");
    assert!(keys.iter().any(|key| key.as_str() == THREADS));
    assert!(keys.iter().any(|key| is_message_shard(key.as_str())));
    assert_eq!(store.list_threads(&scope()).unwrap(), [posted.thread]);
    assert_eq!(store.list_messages(&scope()).unwrap(), [posted.message]);
}
