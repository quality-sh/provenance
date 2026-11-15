//! One record history of the six reviewed kinds, as a graph of commits
//! and the saved working copy after the last commit.
//!
//! The record texts come from the writes of the store over memory storage,
//! and from edits of their JSON lines. The history holds:
//! - `create`: a record of each kind, and `source_gone`, in the scope
//!   `default`; `req_b` in the scope `broken`. The Topic file is at another
//!   path.
//! - `rename`: the Topic file moves to its path.
//! - `start`: a Discussion of `req_a`; no record file changes.
//! - `garble` writes bytes that are not UTF-8 to the Requirement file of
//!   `broken`, and `mend` writes its text again.
//! - `alice` and `bob` change records on two lines, and `merge` joins them:
//!   it keeps the side that changed the Requirement, the Question and the
//!   Resolution, and gives the Rule the changes of both sides.
//! - `outcome` commits only the Discussion file after a save of `req_a`
//!   from its Discussion, and `record` then commits the other files that
//!   the save changed.
//! - `delete` removes `source_gone`.
//! - The working copy holds one more save of `req_a`.

use camino::Utf8PathBuf;
use provenance_core::threads::DiscussionOrigin;
use provenance_core::{
    QuestionStatus, RequirementStatus, ResolutionMethod, ResolutionStatus, RuleSeverity,
    RuleStatus, ScopeId, SourceType, StableId, TopicStatus,
};
use provenance_store::repository::{Clock, IdSource, Repository, RepositoryIdentity};
use provenance_store::review::history::contract::{Commit, Entry};
use provenance_store::settings::Settings;
use provenance_store::state_store::{
    CreateQuestionInput, CreateRequirementInput, CreateResolutionInput, CreateRuleInput,
    CreateSourceInput, CreateTopicInput, StateStore,
};
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use provenance_store::storage::{StateKey, StateStorage as _};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The directory of the state below the working tree.
pub const STATE: &str = ".provenance/state/";

/// The files of one commit or of the working copy, keyed by their path
/// below the working tree.
pub type Tree = BTreeMap<String, Vec<u8>>;

/// The commits, parents first, and the saved working copy.
pub struct History {
    pub commits: Vec<Commit>,
    pub working: Tree,
}

impl History {
    /// The working copy as the entries of a state storage.
    pub fn state(&self) -> BTreeMap<StateKey, Vec<u8>> {
        self.working
            .iter()
            .map(|(path, bytes)| {
                let key = path.strip_prefix(STATE).expect("each file is state");
                (StateKey::new(key).unwrap(), bytes.clone())
            })
            .collect()
    }
}

/// Where the writes of the fixture get their times and ids.
pub struct Sources {
    pub clock: Arc<dyn Clock>,
    pub ids: Arc<dyn IdSource>,
}

struct Graph {
    trees: Vec<(String, Tree)>,
    commits: Vec<Commit>,
    time: i64,
}

impl Graph {
    fn tree(&self, name: &str) -> Tree {
        self.trees
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, tree)| tree.clone())
            .expect("a known commit")
    }

    /// Adds a commit of `tree` by `author`, one minute after the last one.
    fn commit(&mut self, name: &str, parents: &[&str], author: &str, tree: Tree) {
        self.time += 60;
        let base = parents
            .first()
            .map_or_else(Tree::new, |first| self.tree(first));
        let mut commit = Commit::new(name, parents, author, self.time);
        for (path, bytes) in &tree {
            if base.get(path) != Some(bytes) {
                commit = commit.with_entry(path, Entry::new(bytes.clone(), false));
            }
        }
        for path in base.keys().filter(|path| !tree.contains_key(*path)) {
            commit = commit.without_file(path);
        }
        self.commits.push(commit);
        self.trees.push((name.to_owned(), tree));
    }
}

fn scope(name: &str) -> ScopeId {
    ScopeId::new(name).unwrap()
}

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

/// A repository over memory storage that holds `tree`.
fn repository(tree: &Tree, sources: &Sources) -> (Repository, Arc<MemoryStateStorage>) {
    let state = tree
        .iter()
        .map(|(path, bytes)| {
            let key = path.strip_prefix(STATE).unwrap();
            (StateKey::new(key).unwrap(), bytes.clone())
        })
        .collect();
    let storage = Arc::new(MemoryStateStorage::new(state));
    let identity = RepositoryIdentity::new(
        Utf8PathBuf::from("/history"),
        scope("default"),
        Settings::default(),
    )
    .with_clock(Arc::clone(&sources.clock))
    .with_ids(Arc::clone(&sources.ids));
    let repository = Repository::with_storage(
        identity,
        storage.clone(),
        Arc::new(MemorySectionLock::default()),
        None,
    )
    .unwrap();
    (repository, storage)
}

/// The files of `storage`, keyed by their path below the working tree.
fn files(storage: &MemoryStateStorage) -> Tree {
    storage
        .read_view()
        .unwrap()
        .into_entries()
        .into_iter()
        .map(|(key, bytes)| (format!("{STATE}{}", key.as_str()), bytes))
        .collect()
}

/// The path of the record file of `kind` in `scope`, found by the record
/// `id` in it.
fn file_of(tree: &Tree, scope: &str, id: &str) -> String {
    let marker = format!("\"id\":\"{id}\"");
    tree.iter()
        .find(|(path, bytes)| {
            path.starts_with(&format!("{STATE}scopes/{scope}/"))
                && String::from_utf8_lossy(bytes).contains(&marker)
        })
        .map(|(path, _)| path.clone())
        .expect("the record is in a file")
}

/// `tree` with the record `id` in its file changed by `change`, or removed
/// when `change` gives `None`.
fn edited(tree: &Tree, scope: &str, id: &str, change: impl Fn(Value) -> Option<Value>) -> Tree {
    let path = file_of(tree, scope, id);
    let text = String::from_utf8(tree[&path].clone()).unwrap();
    let mut out = String::new();
    for line in text.lines() {
        let record: Value = serde_json::from_str(line).unwrap();
        let record = if record["id"] == json!(id) {
            change(record)
        } else {
            Some(record)
        };
        if let Some(record) = record {
            out.push_str(&serde_json::to_string(&record).unwrap());
            out.push('\n');
        }
    }
    let mut tree = tree.clone();
    tree.insert(path, out.into_bytes());
    tree
}

/// `tree` with the field `name` of the record `id` set to `value`.
fn with_field(tree: &Tree, scope: &str, id: &str, name: &str, value: &str) -> Tree {
    edited(tree, scope, id, |mut record| {
        record[name] = json!(value);
        Some(record)
    })
}

fn manifest() -> Tree {
    let mut manifest = provenance_core::Manifest::default_with_scope(
        scope("default"),
        provenance_core::RepoPathPrefix::new("."),
    );
    manifest.scopes.push(provenance_core::Scope {
        id: scope("broken"),
        path_prefix: provenance_core::RepoPathPrefix::new("broken"),
    });
    Tree::from([(
        format!("{STATE}manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )])
}

fn requirement(scope_name: &str, id: &str, statement: &str) -> CreateRequirementInput {
    CreateRequirementInput {
        scope_id: scope(scope_name),
        id: sid(id),
        statement: statement.into(),
        description: None,
        status: RequirementStatus::Active,
        refines: Vec::new(),
        depends_on: Vec::new(),
        supersedes: Vec::new(),
        spawned_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

fn source(id: &str, name: &str) -> CreateSourceInput {
    CreateSourceInput {
        scope_id: scope("default"),
        id: sid(id),
        name: name.into(),
        source_type: SourceType::Document,
        url: None,
        reference: None,
        commit_pin: None,
        supersedes: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

/// A record of each kind in `default`, and `req_b` in `broken`.
fn create(store: &StateStore) {
    let default = scope("default");
    store
        .create_requirement(requirement(
            "default",
            "req_a",
            "The system stores records.",
        ))
        .unwrap();
    store
        .create_requirement(requirement("broken", "req_b", "The system reads records."))
        .unwrap();
    store
        .create_rule(CreateRuleInput {
            archived_in_commit: None,
            scope_id: default.clone(),
            id: sid("rule_a"),
            name: None,
            description: None,
            requirement_ids: vec![sid("req_a")],
            resolution_ids: Vec::new(),
            statement: "Each record is stored.".into(),
            status: RuleStatus::Active,
            severity: RuleSeverity::High,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_source(source("source_a", "Storage notes"))
        .unwrap();
    store
        .create_source(source("source_gone", "Old notes"))
        .unwrap();
    store
        .create_topic(CreateTopicInput {
            scope_id: default.clone(),
            id: sid("topic_a"),
            requirement_id: sid("req_a"),
            title: "Storage".into(),
            status: TopicStatus::Open,
            links: Vec::new(),
        })
        .unwrap();
    store
        .create_question(CreateQuestionInput {
            depends_on: Vec::new(),
            scope_id: default.clone(),
            id: sid("question_a"),
            topic_id: sid("topic_a"),
            question: "How long are records stored?".into(),
            resolution_method: ResolutionMethod::Research,
            status: QuestionStatus::Open,
            answer: None,
            links: Vec::new(),
            resolution_id: None,
            contradicts: None,
        })
        .unwrap();
    store
        .create_resolution(CreateResolutionInput {
            scope_id: default,
            id: sid("resolution_a"),
            title: "Storage time".into(),
            requirement_ids: vec![sid("req_a")],
            supersedes: Vec::new(),
            position: "Records are stored.".into(),
            rationale: "Readers need old records.".into(),
            status: ResolutionStatus::Draft,
            context: None,
            inputs: Vec::new(),
            made_by: None,
            approved_by: None,
            approved_at: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
}

/// A save of the description of `req_a`.
fn describe(store: &StateStore, description: &str) -> provenance_store::review::SaveRequirement {
    let etag = store
        .requirement_edit_state(&scope("default"), &sid("req_a"))
        .unwrap()
        .etag;
    serde_json::from_value(json!({
        "actor": "ben",
        "expected_etag": etag,
        "update": {"scope_id": "default", "id": "req_a", "description": description},
        "relationships": null
    }))
    .unwrap()
}

/// Builds the history.
pub fn history(sources: &Sources) -> History {
    let mut graph = Graph {
        trees: Vec::new(),
        commits: Vec::new(),
        time: 1_700_000_000,
    };
    let (repository_, storage) = repository(&manifest(), sources);
    let store = repository_.store();
    create(&store);
    let created = files(&storage);
    let topics = file_of(&created, "default", "topic_a");
    let mut moved = created.clone();
    let topic_file = moved.remove(&topics).unwrap();
    moved.insert(topics.replace("/topics/", "/topics/old_"), topic_file);
    graph.commit("create", &[], "Reviewer", moved);
    graph.commit("rename", &["create"], "Reviewer", created);

    let discussion = store
        .write_discussion(
            serde_json::from_value(json!({
                "scope_id": "default",
                "parent": {"node_type": "requirement", "node_id": "req_a"},
                "actor": "ben",
                "declared_by": null,
                "action": {"kind": "start", "role": "user", "body": "Keep records for one year."}
            }))
            .unwrap(),
        )
        .unwrap();
    let origin = DiscussionOrigin {
        discussion_id: discussion.discussion_id.clone(),
        thread_id: discussion.thread_id.clone(),
        message_id: discussion.root_message_id,
    };
    graph.commit("start", &["rename"], "Reviewer", files(&storage));

    let started = graph.tree("start");
    let broken = file_of(&started, "broken", "req_b");
    let mut garbled = started.clone();
    garbled.insert(broken, vec![0xff, 0xfe, b'\n']);
    graph.commit("garble", &["start"], "Reviewer", garbled);
    graph.commit("mend", &["garble"], "Reviewer", started);

    let base = graph.tree("mend");
    let alice = with_field(
        &base,
        "default",
        "req_a",
        "statement",
        "The system stores records for one year.",
    );
    let alice = with_field(
        &alice,
        "default",
        "question_a",
        "question",
        "Are records kept for one year?",
    );
    let alice = with_field(
        &alice,
        "default",
        "rule_a",
        "description",
        "Alice describes the rule.",
    );
    graph.commit("alice", &["mend"], "alice", alice.clone());
    let bob = with_field(
        &base,
        "default",
        "rule_a",
        "statement",
        "Each record is stored once.",
    );
    let bob = with_field(
        &bob,
        "default",
        "resolution_a",
        "position",
        "Records are stored for one year.",
    );
    graph.commit("bob", &["mend"], "bob", bob.clone());
    let mut merged = bob;
    for id in ["req_a", "question_a"] {
        let path = file_of(&alice, "default", id);
        merged.insert(path.clone(), alice[&path].clone());
    }
    let merged = with_field(
        &merged,
        "default",
        "rule_a",
        "description",
        "Alice describes the rule.",
    );
    graph.commit("merge", &["bob", "alice"], "merger", merged.clone());

    let (repository_, storage) = repository(&merged, sources);
    let store = repository_.store();
    store
        .save_requirement_from_discussion(describe(&store, "Text A."), &origin)
        .unwrap();
    let saved = files(&storage);
    let discussions = saved
        .keys()
        .find(|path| path.ends_with("/discussions.jsonl") && merged.get(*path) != saved.get(*path))
        .expect("the save changed the Discussion file");
    let mut outcome = merged.clone();
    outcome.insert(discussions.clone(), saved[discussions].clone());
    graph.commit("outcome", &["merge"], "carol", outcome);
    graph.commit("record", &["outcome"], "dave", saved.clone());

    let deleted = edited(&saved, "default", "source_gone", |_| None);
    graph.commit("delete", &["record"], "erin", deleted.clone());

    let (repository_, storage) = repository(&deleted, sources);
    let store = repository_.store();
    store.save_requirement(describe(&store, "Text B.")).unwrap();
    History {
        commits: graph.commits,
        working: files(&storage),
    }
}
