//! What a repository is, apart from where its state lives: the identity
//! and the commit history of its working tree.

use crate::dictionary_reference::ProjectDictionary;
use crate::settings::Settings;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::ScopeId;
use provenance_macros::rule;
use std::sync::Arc;

/// The time source of the writes of a repository.
pub trait Clock: Send + Sync + std::fmt::Debug {
    /// The current time.
    fn now(&self) -> time::OffsetDateTime;
}

/// The source of the random identities that writes give new records.
pub trait IdSource: Send + Sync + std::fmt::Debug {
    /// A new UUID.
    fn new_uuid(&self) -> uuid::Uuid;
}

/// The system clock, read through the time crate.
#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    /// The time crate reads the clock through JavaScript on wasm32, so this
    /// also works in a Worker.
    #[rule("rule_store_time_works_in_a_worker")]
    fn now(&self) -> time::OffsetDateTime {
        time::OffsetDateTime::now_utc()
    }
}

/// Random version 4 UUIDs, so independent stores can merge their records.
#[derive(Debug, Default)]
pub struct RandomIds;

impl IdSource for RandomIds {
    fn new_uuid(&self) -> uuid::Uuid {
        uuid::Uuid::new_v4()
    }
}

/// The commit history of the working tree of a repository.
///
/// `open` gives a repository the history of its git work tree. With no
/// history, a write stamps no commit and the archive check finds no gap, as
/// outside a work tree.
pub trait RepositoryHistory: Send + Sync + std::fmt::Debug {
    /// The commit of `HEAD`, or `None` when there is no commit.
    fn head(&self) -> anyhow::Result<Option<String>>;
    /// `HEAD` when its history is complete; `None` in a shallow clone.
    fn complete_head(&self) -> anyhow::Result<Option<String>>;
    /// Whether `commit` names a commit of the history.
    fn commit_exists(&self, commit: &str) -> anyhow::Result<bool>;
    /// Whether `commit` is in the history of `head`.
    fn is_ancestor(&self, commit: &str, head: &str) -> anyhow::Result<bool>;
}

/// What a repository is, apart from where its state lives.
///
/// The working tree, the selected scope, the settings read at open, the
/// project dictionary of statement checks and the commit history. It has no
/// store constructor, so a holder cannot reach canonical state through it.
#[derive(Clone, Debug)]
pub struct RepositoryIdentity {
    root: Utf8PathBuf,
    scope: ScopeId,
    settings: Arc<Settings>,
    dictionary: Option<Arc<ProjectDictionary>>,
    history: Option<Arc<dyn RepositoryHistory>>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdSource>,
}

impl RepositoryIdentity {
    /// The identity of a repository whose working tree is `root`, with the
    /// selected scope and the settings that the host read.
    /// The identity has no project dictionary and no history until the
    /// `with_` methods give them. It reads the system clock and makes random
    /// IDs until `with_clock` and `with_ids` give other sources.
    pub fn new(root: Utf8PathBuf, scope: ScopeId, settings: Settings) -> Self {
        Self {
            root,
            scope,
            settings: Arc::new(settings),
            dictionary: None,
            history: None,
            clock: Arc::new(SystemClock),
            ids: Arc::new(RandomIds),
        }
    }

    /// This identity with the clock that its writes read.
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// This identity with the source of the IDs that its writes make.
    #[must_use]
    pub fn with_ids(mut self, ids: Arc<dyn IdSource>) -> Self {
        self.ids = ids;
        self
    }

    /// The clock that the writes read.
    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }

    /// The source of the IDs that the writes make.
    pub fn ids(&self) -> &Arc<dyn IdSource> {
        &self.ids
    }

    /// This identity with the commit history of its working tree.
    #[must_use]
    pub fn with_history(mut self, history: Arc<dyn RepositoryHistory>) -> Self {
        self.history = Some(history);
        self
    }

    /// The commit history of the working tree, if the host gave one.
    pub fn history(&self) -> Option<&Arc<dyn RepositoryHistory>> {
        self.history.as_ref()
    }

    /// This identity with the project dictionary that the host resolved.
    #[must_use]
    pub fn with_dictionary(mut self, dictionary: Option<ProjectDictionary>) -> Self {
        self.dictionary = dictionary.map(Arc::new);
        self
    }

    /// The project dictionary that statement checks use, if the project has
    /// one that loaded.
    pub fn dictionary(&self) -> Option<Arc<ProjectDictionary>> {
        self.dictionary.clone()
    }

    /// The canonical working-tree root.
    pub fn working_tree(&self) -> &Utf8Path {
        &self.root
    }

    /// The scope the handle selected at open.
    pub const fn scope(&self) -> &ScopeId {
        &self.scope
    }

    /// The settings read once at open.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }
}
