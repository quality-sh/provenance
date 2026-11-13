//! The state that every case starts from, and the records of a final state
//! that the laws read.

use crate::generate::{
    self, Target, Value, DECIDER, DETAIL_REQUIREMENT, FIRST_REQUIREMENT, FIRST_RULE, TOPIC,
};
use crate::identity::{identity, identity_with_ids, scope, stable, INITIAL_STATE_IDS};
use provenance_core::TopicStatus;
use provenance_store::repository::Repository;
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use provenance_store::storage::{StateKey, StateStorage};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// The state that every case starts from and the etag of its Requirement.
pub struct InitialState {
    pub entries: BTreeMap<StateKey, Vec<u8>>,
    pub first_etag: String,
}

/// The manifest (with one actor that can decide a review), two
/// Requirements, one Rule and one Topic. The second Requirement refines the
/// first, so an archive of the first archives the second in a stage.
pub fn initial_state() -> InitialState {
    use provenance_core::{Manifest, RepoPathPrefix};
    let mut manifest = Manifest::default_with_scope(scope(), RepoPathPrefix::new("."));
    manifest.disposition_actor_ids = vec![DECIDER.into()];
    let key = |value: &str| StateKey::new(value).expect("a state key");
    let shard = |rows: &[serde_json::Value]| {
        let mut bytes = Vec::new();
        for row in rows {
            bytes.extend(serde_json::to_vec(row).expect("JSON"));
            bytes.push(b'\n');
        }
        bytes
    };
    let requirements = [
        serde_json::json!({
            "scope_id": generate::SCOPE, "id": FIRST_REQUIREMENT,
            "statement": "Share links expire", "status": "active",
        }),
        serde_json::json!({
            "scope_id": generate::SCOPE, "id": DETAIL_REQUIREMENT,
            "statement": "Share links expire after one day", "status": "active",
            "refines": [FIRST_REQUIREMENT],
        }),
    ];
    let rule = serde_json::json!({
        "scope_id": generate::SCOPE, "id": FIRST_RULE,
        "statement": "Share links expire", "status": "active", "severity": "medium",
        "requirement_ids": [FIRST_REQUIREMENT],
    });
    let entries = BTreeMap::from([
        (
            key("manifest.json"),
            serde_json::to_vec(&manifest).expect("JSON"),
        ),
        (
            key("scopes/default/requirements/req.jsonl"),
            shard(&requirements),
        ),
        (key("scopes/default/rules/rule.jsonl"), shard(&[rule])),
    ]);
    let storage = Arc::new(MemoryStateStorage::new(entries));
    let repository = Repository::with_storage(
        identity_with_ids(INITIAL_STATE_IDS),
        storage.clone(),
        Arc::new(MemorySectionLock::default()),
        None,
    )
    .expect("the initial state opens");
    let store = repository.store();
    store
        .create_topic(provenance_store::state_store::CreateTopicInput {
            scope_id: scope(),
            id: stable(TOPIC),
            requirement_id: stable(FIRST_REQUIREMENT),
            title: "Who owns a share link".into(),
            status: TopicStatus::Open,
            links: Vec::new(),
        })
        .expect("the Topic is written");
    let first_etag = store
        .requirement_edit_state(&scope(), &stable(FIRST_REQUIREMENT))
        .expect("the Requirement has an etag")
        .etag;
    InitialState {
        entries: storage.read_view().expect("a view").into_entries(),
        first_etag,
    }
}

/// Reads every record kind that the operations write.
pub fn read_all(repository: &Repository) -> anyhow::Result<()> {
    let view = repository.view()?;
    view.list_requirements(&scope())?;
    view.list_topics(&scope())?;
    view.list_messages(&scope())?;
    view.list_verification_bindings(&scope())?;
    Ok(())
}

/// A repository over a copy of `entries`.
pub fn open_entries(entries: &BTreeMap<StateKey, Vec<u8>>) -> anyhow::Result<Repository> {
    Ok(Repository::with_storage(
        identity(None),
        Arc::new(MemoryStateStorage::new(entries.clone())),
        Arc::new(MemorySectionLock::default()),
        None,
    )?)
}

/// The records of the final state that the laws follow.
#[derive(Debug, Default)]
pub struct FinalRecords {
    /// Each Requirement with its description.
    pub descriptions: BTreeMap<String, Option<String>>,
    /// The status of the Topic.
    pub topic_status: Option<TopicStatus>,
    /// The holder of the Topic claim.
    pub claim: Option<String>,
    pub messages: BTreeSet<String>,
}

impl FinalRecords {
    /// Tells if `target` holds `value`.
    pub fn holds(&self, target: &Target, value: &Value) -> bool {
        match (target, value) {
            (Target::Requirement(id), Value::Absent) => !self.descriptions.contains_key(id),
            (Target::Requirement(id), Value::Requirement { description }) => {
                self.descriptions.get(id) == Some(description)
            }
            (Target::Topic, Value::Topic { status, claim }) => {
                self.topic_status == Some(*status) && self.claim == *claim
            }
            _ => false,
        }
    }
}

/// Reads the records that the laws follow from `entries`.
pub fn final_records(entries: &BTreeMap<StateKey, Vec<u8>>) -> anyhow::Result<FinalRecords> {
    let repository = open_entries(entries)?;
    let view = repository.view()?;
    let topic = view
        .list_topics(&scope())?
        .into_iter()
        .find(|topic| topic.id.as_str() == TOPIC);
    Ok(FinalRecords {
        descriptions: view
            .list_requirements(&scope())?
            .into_iter()
            .map(|requirement| (requirement.id.as_str().to_owned(), requirement.description))
            .collect(),
        topic_status: topic.as_ref().map(|topic| topic.status),
        claim: topic.and_then(|topic| topic.claimed_by),
        messages: view
            .list_messages(&scope())?
            .into_iter()
            .map(|message| message.id.as_str().to_owned())
            .collect(),
    })
}
