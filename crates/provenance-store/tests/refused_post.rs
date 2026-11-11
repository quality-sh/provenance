//! A message post over the memory adapter, where the persistence refuses
//! each write set that changes a Message shard. The refused post lands no
//! Thread and no Message, and the next post makes one of each.

#[path = "support/memory.rs"]
mod memory;

use memory::{id, initial_state, scope};
use provenance_core::{MessageRole, NodeType, ThreadParent};
use provenance_macros::verifies;
use provenance_store::repository::{Repository, RepositoryIdentity};
use provenance_store::settings::Settings;
use provenance_store::state_store::PostMessageInput;
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use provenance_store::storage::{PublishFailure, StatePersistence, WriteSet};
use provenance_store::storage::{StateKey, StateStorage};
use provenance_store::write_error::{WriteError, WriteFailure};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::Arc;

const THREAD_DIRECTORY: &str = "scopes/default/threads/";
const THREADS: &str = "scopes/default/threads/threads.jsonl";

/// Refuses each set that puts a Message shard, while `refuse` is set.
#[derive(Debug, Default)]
struct RefuseMessages {
    refuse: AtomicBool,
}

impl StatePersistence for RefuseMessages {
    fn persist(&self, writes: &WriteSet) -> Result<(), PublishFailure> {
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

/// Whether the state holds a Thread shard or a Message shard.
fn holds_threads(storage: &MemoryStateStorage) -> bool {
    let view = storage.read_view().unwrap();
    let mut keys = view.entries().keys().map(StateKey::as_str);
    keys.any(|key| key.starts_with(THREAD_DIRECTORY))
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
    assert!(!holds_threads(&storage));
    assert!(store.list_threads(&scope()).unwrap().is_empty());
    assert!(store.list_messages(&scope()).unwrap().is_empty());

    persistence.refuse.store(false, SeqCst);
    let posted = store.post_thread_message(post()).unwrap();
    assert_eq!(store.list_threads(&scope()).unwrap(), [posted.thread]);
    assert_eq!(store.list_messages(&scope()).unwrap(), [posted.message]);
}
