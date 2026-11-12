//! A message post over the memory adapter, where the persistence keeps each
//! write set that it receives and refuses each set that changes a Message
//! shard. The post publishes one whole set, so the refused post lands no
//! Thread and no Message, and the next post makes one of each.

#[path = "support/memory.rs"]
mod memory;

use memory::{id, initial_state, scope};
use provenance_core::{Message, MessageRole, NodeType, Thread, ThreadParent};
use provenance_macros::verifies;
use provenance_store::repository::{Repository, RepositoryIdentity};
use provenance_store::settings::Settings;
use provenance_store::state_store::PostMessageInput;
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use provenance_store::storage::{Atomicity, PublishFailure, StatePersistence, WriteSet};
use provenance_store::storage::{StateKey, StateStorage};
use provenance_store::write_error::{WriteError, WriteFailure};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::{Arc, Mutex};

const THREAD_DIRECTORY: &str = "scopes/default/threads/";
const THREADS: &str = "scopes/default/threads/threads.jsonl";

/// Keeps each set that it receives. Refuses each set that puts a Message
/// shard, while `refuse` is set.
#[derive(Debug, Default)]
struct RefuseMessages {
    refuse: AtomicBool,
    received: Mutex<Vec<WriteSet>>,
}

impl RefuseMessages {
    /// The sets that the persistence received, from the first.
    fn take(&self) -> Vec<WriteSet> {
        std::mem::take(&mut *self.received.lock().unwrap())
    }
}

impl StatePersistence for RefuseMessages {
    fn persist(&self, writes: &WriteSet) -> Result<(), PublishFailure> {
        self.received.lock().unwrap().push(writes.clone());
        let keys = writes.puts().keys();
        if self.refuse.load(SeqCst) && keys.map(StateKey::as_str).any(is_message_shard) {
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
    let persistence = Arc::new(RefuseMessages::default());
    let storage = Arc::new(MemoryStateStorage::loaded(
        initial_state(),
        persistence.clone(),
    ));
    let working_tree = camino::Utf8PathBuf::from("/simulated");
    let identity = RepositoryIdentity::new(working_tree, scope(), Settings::default());
    let repository = Repository::with_storage(
        identity,
        storage.clone(),
        Arc::new(MemorySectionLock::default()),
    )
    .unwrap();
    let store = repository.store();

    persistence.refuse.store(true, SeqCst);
    let error = store.post_thread_message(post()).unwrap_err();
    assert!(matches!(
        WriteError::from(error).safe(),
        WriteFailure::WriteFailed { .. }
    ));
    let attempts = persistence.take();
    assert_eq!(attempts.len(), 1, "one attempt: {attempts:?}");
    assert_eq!(thread_keys(&storage), [] as [StateKey; 0]);
    assert_eq!(store.list_threads(&scope()).unwrap(), [] as [Thread; 0]);
    assert_eq!(store.list_messages(&scope()).unwrap(), [] as [Message; 0]);

    persistence.refuse.store(false, SeqCst);
    let posted = store.post_thread_message(post()).unwrap();
    let sets = persistence.take();
    assert_eq!(sets.len(), 1, "the accepted post gave one set: {sets:?}");
    assert_eq!(sets[0].atomicity(), Atomicity::WholeSet);
    let keys: Vec<_> = sets[0].puts().keys().cloned().collect();
    assert_eq!(keys, thread_keys(&storage));
    assert!(keys.iter().any(|key| key.as_str() == THREADS));
    assert!(keys.iter().any(|key| is_message_shard(key.as_str())));
    assert_eq!(keys.len(), 2, "{keys:?}");
    assert_eq!(store.list_threads(&scope()).unwrap(), [posted.thread]);
    assert_eq!(store.list_messages(&scope()).unwrap(), [posted.message]);
}
