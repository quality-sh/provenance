use super::initialized_store;
use provenance_core::{MessageRole, StableId};
use provenance_macros::verifies;

#[test]
#[verifies("rule_message_id_is_random", examples)]
#[verifies("rule_message_time_follows_write_clock", examples)]
fn independent_stores_write_distinct_uuid_messages_at_the_write_clock() {
    use crate::state_store::PostMessageInput;
    use provenance_core::{NodeType, ThreadParent};
    let (_shared, shared, _, existing) = posted_store();
    let (_a, a, scope) = initialized_store();
    let (_b, b, _) = initialized_store();
    for store in [&a, &b] {
        copy_state(&shared.layout.state_dir(), &store.layout.state_dir());
        assert_eq!(store.list_messages(&scope).unwrap(), vec![existing.clone()]);
    }
    let post = |store: &crate::state_store::StateStore, body: &str| {
        store
            .post_thread_message(PostMessageInput {
                scope_id: scope.clone(),
                parent: ThreadParent {
                    node_type: NodeType::Requirement,
                    node_id: StableId::new("req_overtime").unwrap(),
                },
                role: MessageRole::User,
                body: body.into(),
            })
            .unwrap()
            .message
    };
    let before = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
    let first = post(&a, "first");
    let other = post(&b, "other");
    let after = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
    assert_ne!(first.id, other.id);
    for message in [&first, &other] {
        let id = uuid::Uuid::parse_str(message.id.as_str().strip_prefix("msg_").unwrap()).unwrap();
        assert_eq!(id.get_version_num(), 4);
        let earliest = before.max(i128::from(existing.created_at) + 1);
        let latest = after.max(i128::from(existing.created_at) + 1);
        assert!((earliest..=latest).contains(&i128::from(message.created_at)));
    }
    let second = post(&a, "second");
    assert!(second.created_at > first.created_at);
    let mut combined = a.list_messages(&scope).unwrap();
    combined.push(other);
    crate::jsonl::write_jsonl_atomic(&crate::shards::messages_path(&a.layout, &scope), &combined)
        .unwrap();
    let merged = a.list_messages(&scope).unwrap();
    assert_eq!(merged.len(), 4);
    for message in [existing, first, second] {
        assert!(merged.contains(&message));
    }
    assert!(merged.iter().any(|message| message.body == "other"));
}

fn copy_state(source: &camino::Utf8Path, destination: &camino::Utf8Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let child = camino::Utf8PathBuf::from_path_buf(entry.path()).unwrap();
        let target = destination.join(entry.file_name().to_string_lossy().as_ref());
        if entry.file_type().unwrap().is_dir() {
            copy_state(&child, &target);
        } else {
            std::fs::copy(child, target).unwrap();
        }
    }
}

fn posted_store() -> (
    tempfile::TempDir,
    crate::state_store::StateStore,
    provenance_core::ScopeId,
    provenance_core::Message,
) {
    use crate::state_store::PostMessageInput;
    use provenance_core::{NodeType, ThreadParent};
    let (dir, store, scope) = super::seeded_requirement_store();
    let message = store
        .post_thread_message(PostMessageInput {
            scope_id: scope.clone(),
            parent: ThreadParent {
                node_type: NodeType::Requirement,
                node_id: StableId::new("req_overtime").unwrap(),
            },
            role: MessageRole::User,
            body: "first".into(),
        })
        .unwrap()
        .message;
    (dir, store, scope, message)
}

