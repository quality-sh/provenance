//! The contract harness of the remote adapter: one project of a running
//! object fixture, read and written over HTTP.
//!
//! The harness seeds with `PUT` on the state path, which replaces the whole
//! state, and breaks a key with the fixture's `PUT <session>/failing-key
//! {"key": <key>}`. A real object has no such route.

use super::{expected, Harness, StateStorage};
use crate::storage::remote::wire::{self, SeedBody, MAIN_SESSION};
use crate::storage::remote::RemoteStateStorage;
use crate::storage::StateKey;
use std::collections::BTreeMap;

/// One project of the object fixture at a base, in session `main`.
#[derive(Debug)]
pub struct Remote {
    storage: RemoteStateStorage,
}

impl Remote {
    /// Makes `project` with an empty state at the object fixture at `base`.
    ///
    /// # Panics
    /// When the fixture does not make the project.
    pub fn new(base: &str, project: &str) -> Self {
        let harness = Self {
            storage: RemoteStateStorage::new(base, project, MAIN_SESSION).unwrap(),
        };
        harness.replace(&BTreeMap::new());
        harness
    }

    /// The adapter of the project.
    pub const fn storage(&self) -> &RemoteStateStorage {
        &self.storage
    }

    fn replace(&self, entries: &BTreeMap<StateKey, Vec<u8>>) {
        let body = SeedBody {
            entries: wire::encode_entries(entries),
        };
        self.expect_no_content("PUT", "/state", &serde_json::to_vec(&body).unwrap());
    }

    fn expect_no_content(&self, method: &str, path: &str, body: &[u8]) {
        let answer = self
            .storage
            .send(method, path, &[], Some(body))
            .map_err(|failure| failure.into_error())
            .unwrap();
        assert_eq!(answer.status, 204, "{:#}", answer.failure_error());
    }
}

impl Harness for Remote {
    fn seed(&mut self, entries: &[(&str, &[u8])]) {
        let mut state = self.storage.read_view().unwrap().into_entries();
        state.extend(expected(entries));
        self.replace(&state);
    }

    fn with_storage<R>(&self, call: impl FnOnce(&dyn StateStorage) -> R) -> R {
        call(&self.storage)
    }

    fn break_key(&mut self, key: &str) {
        let body = serde_json::json!({ "key": key }).to_string();
        self.expect_no_content("PUT", "/failing-key", body.as_bytes());
    }
}
