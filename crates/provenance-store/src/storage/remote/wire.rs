//! The state wire: the paths and JSON bodies that the remote adapter and a
//! project's object exchange.
//!
//! The state of one session of a project is at
//! `<base>/projects/<project>/sessions/<session>/state`:
//!
//! - `GET` gives `200 {revision, entries}` with the revision as its `ETag`,
//!   or `304` when `If-None-Match` names the stored revision.
//! - `POST` with a [`PublishBody`] lands a write set and gives `204`.
//! - `PUT` with a [`SeedBody`] replaces the whole state and gives `204`. It
//!   makes the project when it does not exist.
//!
//! A failure gives the status of its [`StateFailure`] and the failure as the
//! body. Bytes go on the wire as standard base64 with padding.

use super::super::{Atomicity, Revision, StateKey, WriteSet};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The one session that an object has until shaping sessions exist.
pub const MAIN_SESSION: &str = "main";

/// True when `name` can name a project or a session: 1 to 64 ASCII letters,
/// digits, `_` and `-`.
pub fn is_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

/// The path of `session` of `project`, below the object base.
pub fn session_path(project: &str, session: &str) -> String {
    format!("/projects/{project}/sessions/{session}")
}

/// The path of the state of `session` of `project`, below the object base.
pub fn state_path(project: &str, session: &str) -> String {
    format!("{}/state", session_path(project, session))
}

/// The `ETag` value of `revision`: the revision in double quotes.
pub fn entity_tag(revision: &str) -> String {
    format!("\"{revision}\"")
}

/// The body of `200` to `GET`: every key with its bytes, and the revision of
/// those keys and bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateBody {
    pub revision: String,
    pub entries: BTreeMap<String, String>,
}

/// The body of `PUT`: the whole state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeedBody {
    pub entries: BTreeMap<String, String>,
}

/// The body of `POST`: one write set. With `base_revision`, the object
/// refuses the set when its stored state has another revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<String>,
    pub atomicity: WireAtomicity,
    pub puts: BTreeMap<String, String>,
    pub deletes: Vec<String>,
}

/// The atomicity of a write set on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireAtomicity {
    PerKey,
    WholeSet,
}

/// Why the object did not do a state request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum StateFailure {
    /// The object has no such project, or the project has no such session.
    UnknownTarget {},
    /// The request is not a state request: a bad body, key or base64 text.
    InvalidInput { message: String },
    /// The stored state is not at the base revision of the set. No key
    /// landed.
    BaseMoved { base: String, current: String },
    /// The object refused the set. No key landed.
    Refused { message: String },
    /// The failure came after the first key began to land. A per-key set can
    /// have a prefix of its keys landed.
    Started { message: String },
}

impl StateFailure {
    /// The HTTP status of the failure.
    pub const fn status(&self) -> u16 {
        match self {
            Self::UnknownTarget {} => 404,
            Self::InvalidInput { .. } => 400,
            Self::BaseMoved { .. } => 409,
            Self::Refused { .. } => 422,
            Self::Started { .. } => 500,
        }
    }
}

/// The wire text of `bytes`.
pub fn encode(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// The keys and bytes of `entries`, as wire text.
pub fn encode_entries(entries: &BTreeMap<StateKey, Vec<u8>>) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(key, bytes)| (key.as_str().to_owned(), encode(bytes)))
        .collect()
}

/// The keys and bytes of wire `entries`. Refuses a bad key or base64 text.
pub fn decode_entries(
    entries: BTreeMap<String, String>,
) -> Result<BTreeMap<StateKey, Vec<u8>>, String> {
    entries
        .into_iter()
        .map(|(key, text)| {
            let bytes = STANDARD
                .decode(&text)
                .map_err(|error| format!("the bytes of `{key}` are not base64: {error}"))?;
            Ok((decode_key(key)?, bytes))
        })
        .collect()
}

fn decode_key(key: String) -> Result<StateKey, String> {
    StateKey::new(key).map_err(|error| error.to_string())
}

impl StateBody {
    /// The keys and bytes of the body. Refuses a body whose revision is not
    /// the revision of its keys and bytes.
    pub fn decode(self) -> Result<BTreeMap<StateKey, Vec<u8>>, String> {
        let entries = decode_entries(self.entries)?;
        let revision = Revision::of(&entries);
        if revision.as_str() == self.revision {
            Ok(entries)
        } else {
            Err(format!(
                "the state body names revision {} but its entries have revision {}",
                self.revision,
                revision.as_str()
            ))
        }
    }
}

impl PublishBody {
    /// The body of `writes`. The base goes as its revision.
    pub fn of(writes: &WriteSet) -> Self {
        Self {
            base_revision: writes.base().map(|base| base.as_str().to_owned()),
            atomicity: match writes.atomicity() {
                Atomicity::PerKey => WireAtomicity::PerKey,
                Atomicity::WholeSet => WireAtomicity::WholeSet,
            },
            puts: encode_entries(writes.puts()),
            deletes: writes
                .deletes()
                .iter()
                .map(|key| key.as_str().to_owned())
                .collect(),
        }
    }

    /// The write set of the body, with no base, and the base revision.
    /// Refuses a bad key or base64 text.
    pub fn decode(self) -> Result<(Option<String>, WriteSet), String> {
        let mut writes = WriteSet::new(match self.atomicity {
            WireAtomicity::PerKey => Atomicity::PerKey,
            WireAtomicity::WholeSet => Atomicity::WholeSet,
        });
        for (key, bytes) in decode_entries(self.puts)? {
            writes = writes.put(key, bytes);
        }
        for key in self.deletes {
            writes = writes.delete(decode_key(key)?);
        }
        Ok((self.base_revision, writes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An implementation aid with no Rule binding: a name goes into a path
    /// as it is, so a name must need no encoding.
    #[test]
    fn names_are_one_to_64_letters_digits_underscores_and_hyphens() {
        assert!(is_name("a"));
        assert!(is_name("Project_1-b"));
        assert!(is_name(&"x".repeat(64)));
        for name in ["", "a/b", "a.b", "é", " a", &"x".repeat(65)] {
            assert!(!is_name(name), "{name:?}");
        }
    }

    /// An implementation aid with no Rule binding: no Rule records the wire
    /// format. It pins the failure shape that the object must send.
    #[test]
    fn a_failure_names_its_kind_on_the_wire() {
        let moved = StateFailure::BaseMoved {
            base: "sha256:a".into(),
            current: "sha256:b".into(),
        };
        assert_eq!(
            serde_json::to_value(&moved).unwrap(),
            serde_json::json!({"base_moved": {"base": "sha256:a", "current": "sha256:b"}})
        );
        assert_eq!(
            serde_json::to_value(StateFailure::UnknownTarget {}).unwrap(),
            serde_json::json!({"unknown_target": {}})
        );
    }

    /// An implementation aid with no Rule binding: it keeps the adapter from
    /// taking a body whose revision does not match its keys and bytes.
    #[test]
    fn a_state_body_with_another_revision_is_refused() {
        let body = StateBody {
            revision: "sha256:0".into(),
            entries: BTreeMap::from([("manifest.json".into(), encode(b"{}"))]),
        };
        assert!(body.decode().unwrap_err().contains("sha256:0"));
    }
}
