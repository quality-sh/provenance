//! The remote storage adapter against a spawned `object-fixture`: the
//! storage contract, two writers on one base, sessions, and the replay of
//! the state-wire examples that provenance-web replays against the object.

use provenance_macros::verifies;
use provenance_store::storage::contract::remote_harness::Remote;
use provenance_store::storage::contract::{
    a_failed_one_key_set_lands_nothing, a_failed_per_key_set_leaves_a_landed_prefix,
    a_failed_whole_set_lands_no_key, a_one_key_set_lands_all_of_its_bytes,
    a_two_key_whole_set_lands_both_keys, a_whole_set_on_a_moved_base_is_refused,
    empty_state_reads_as_an_empty_view, publish_lands_puts_and_deletes,
    publish_on_a_base_with_changed_bytes_is_refused, publish_on_a_moved_base_is_refused,
    publish_on_the_current_base_lands, publish_refuses_a_put_and_delete_of_one_key,
    revision_follows_every_key_and_byte, view_returns_every_key_with_its_bytes,
};
use provenance_store::storage::remote::RemoteStateStorage;
use provenance_store::storage::{
    Atomicity, PublishFailure, StateKey, StateStorage, StorageError, WriteSet,
};
use serde_json::Value;
use std::io::{BufRead as _, BufReader};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Barrier};

/// A running object fixture. Dropping it closes its input, which stops it.
struct Fixture {
    url: String,
    input: Option<ChildStdin>,
    child: Child,
}

impl Fixture {
    fn spawn() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_object-fixture"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let started: Value = serde_json::from_str(&line).unwrap();
        Self {
            url: started["url"].as_str().unwrap().to_owned(),
            input: child.stdin.take(),
            child,
        }
    }

    /// A new project with an empty state.
    fn project(&self, project: &str) -> Remote {
        Remote::new(&self.url, project)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.input.take());
        self.child.wait().unwrap();
    }
}

fn key(key: &str) -> StateKey {
    StateKey::new(key).unwrap()
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
#[verifies("rule_one_key_write_set_lands_whole", examples)]
#[verifies("rule_failure_after_landing_starts_is_an_unknown_outcome", examples)]
fn the_remote_adapter_publishes_through_the_storage_port() {
    let fixture = Fixture::spawn();
    publish_lands_puts_and_deletes(fixture.project("puts"));
    publish_on_the_current_base_lands(fixture.project("current"));
    publish_refuses_a_put_and_delete_of_one_key(fixture.project("both"));
    a_one_key_set_lands_all_of_its_bytes(fixture.project("bytes"));
    a_failed_one_key_set_lands_nothing(fixture.project("one-key"));
    a_failed_per_key_set_leaves_a_landed_prefix(fixture.project("prefix"));
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
#[verifies("rule_stage_lands_as_one_whole_set", examples)]
fn the_remote_adapter_lands_a_two_key_whole_set_and_rolls_back_a_failed_one() {
    let fixture = Fixture::spawn();
    a_two_key_whole_set_lands_both_keys(fixture.project("two"));
    a_failed_whole_set_lands_no_key(fixture.project("rolled-back"));
}

#[test]
#[verifies("rule_adapter_refuses_a_set_on_a_moved_base", examples)]
fn the_remote_adapter_refuses_a_set_on_a_moved_base() {
    let fixture = Fixture::spawn();
    publish_on_a_moved_base_is_refused(fixture.project("moved"));
    publish_on_a_base_with_changed_bytes_is_refused(fixture.project("changed"));
    a_whole_set_on_a_moved_base_is_refused(fixture.project("whole"));
}

#[test]
#[verifies("rule_canonical_state_is_read_as_one_view", examples)]
fn the_remote_adapter_reads_one_view_of_every_key() {
    let fixture = Fixture::spawn();
    view_returns_every_key_with_its_bytes(fixture.project("view"));
    empty_state_reads_as_an_empty_view(&fixture.project("empty"));
    revision_follows_every_key_and_byte(fixture.project("revision"));
}

/// Two writers that read the same view and publish on it at the same time:
/// one set lands and the other gets `BaseMoved`.
#[test]
#[verifies("rule_adapter_refuses_a_set_on_a_moved_base", examples)]
fn of_two_concurrent_publishes_on_one_base_one_gets_base_moved() {
    let fixture = Fixture::spawn();
    let project = fixture.project("race");
    let barrier = Arc::new(Barrier::new(2));
    let results: Vec<_> = ["scopes/a/rules/one.jsonl", "scopes/a/rules/two.jsonl"]
        .map(|written| {
            let storage = RemoteStateStorage::new(&fixture.url, "race", "main").unwrap();
            let base = Arc::new(storage.read_view().unwrap());
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let writes = WriteSet::new(Atomicity::WholeSet)
                    .on_base(base)
                    .put(key(written), b"x".to_vec());
                barrier.wait();
                storage.publish(writes)
            })
        })
        .into_iter()
        .map(|writer| writer.join().unwrap())
        .collect();
    let landed = results.iter().filter(|result| result.is_ok()).count();
    let moved = results
        .iter()
        .filter(|result| matches!(result, Err(PublishFailure::BaseMoved { .. })))
        .count();
    assert_eq!((landed, moved), (1, 1), "{results:?}");
    assert_eq!(project.storage().read_view().unwrap().entries().len(), 1);
}

/// Only session `main` exists: another session reads as an unknown target
/// and refuses a write.
#[test]
fn a_session_other_than_main_is_an_unknown_target() {
    let fixture = Fixture::spawn();
    let main = fixture.project("sessions");
    let other = RemoteStateStorage::new(&fixture.url, "sessions", "x").unwrap();
    let Err(StorageError::Read(error)) = other.read_view() else {
        panic!("the session `x` gave a view");
    };
    assert!(format!("{error:#}").contains("404"), "{error:#}");
    let writes = WriteSet::new(Atomicity::PerKey).put(key("manifest.json"), b"{}".to_vec());
    assert!(matches!(
        other.publish(writes),
        Err(PublishFailure::Refused(_))
    ));
    assert!(main.storage().read_view().unwrap().entries().is_empty());
}

/// A publish that cannot reach the object is refused: no key can have
/// landed.
#[test]
fn a_publish_to_an_object_that_does_not_answer_is_refused() {
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    let storage = RemoteStateStorage::new(&url, "gone", "main").unwrap();
    assert!(matches!(storage.read_view(), Err(StorageError::Read(_))));
    let writes = WriteSet::new(Atomicity::PerKey).put(key("manifest.json"), b"{}".to_vec());
    assert!(matches!(
        storage.publish(writes),
        Err(PublishFailure::Refused(_))
    ));
}

/// Each `contracts/state-wire` example gets its recorded answers from the
/// fixture, so the examples that provenance-web replays against the object
/// are what the remote adapter speaks.
#[test]
fn each_state_wire_example_replays_against_the_fixture() {
    let fixture = Fixture::spawn();
    let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/contracts/state-wire");
    let mut names: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    names.sort();
    assert!(names.len() >= 5, "{names:?}");
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    for path in names {
        let example: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        for (step, exchange) in example["exchanges"].as_array().unwrap().iter().enumerate() {
            let at = format!("{} exchange {step}", path.display());
            replay(&agent, &fixture.url, exchange, &at);
        }
    }
}

fn replay(agent: &ureq::Agent, url: &str, exchange: &Value, at: &str) {
    let request = &exchange["request"];
    let mut call = agent.request(
        request["method"].as_str().unwrap(),
        &format!("{url}{}", request["path"].as_str().unwrap()),
    );
    for (name, value) in request["headers"].as_object().into_iter().flatten() {
        call = call.set(name, value.as_str().unwrap());
    }
    let answer = match &request["body"] {
        Value::Null => call.call(),
        body => call.send_string(&body.to_string()),
    };
    let answer = match answer {
        Ok(answer) | Err(ureq::Error::Status(_, answer)) => answer,
        Err(error) => panic!("{at}: {error}"),
    };
    let expected = &exchange["response"];
    assert_eq!(
        u64::from(answer.status()),
        expected["status"].as_u64().unwrap(),
        "{at}"
    );
    for (name, value) in expected["headers"].as_object().into_iter().flatten() {
        assert_eq!(answer.header(name), value.as_str(), "{at}: header {name}");
    }
    let text = answer.into_string().unwrap();
    let body = if text.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).unwrap()
    };
    assert_eq!(body, expected["body"], "{at}");
}
