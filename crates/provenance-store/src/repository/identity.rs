//! What a repository is, apart from where its state lives: the identity
//! and the commit history of its working tree.

use crate::dictionary_reference::ProjectDictionary;
use crate::settings::Settings;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::ScopeId;
use std::sync::Arc;

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
}

impl RepositoryIdentity {
    /// The identity of a repository whose working tree is `root`, with the
    /// selected scope and the settings that the host read.
    /// The identity has no project dictionary and no history until the
    /// `with_` methods give them.
    pub fn new(root: Utf8PathBuf, scope: ScopeId, settings: Settings) -> Self {
        Self {
            root,
            scope,
            settings: Arc::new(settings),
            dictionary: None,
            history: None,
        }
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
