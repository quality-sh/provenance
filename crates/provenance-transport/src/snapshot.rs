pub use provenance_store::scope_snapshot::{snapshot, GraphRecords, ScopeSnapshot};
use provenance_store::{repository::Repository, state_store::StateStore};

/// The live store of one opened repository.
#[derive(Debug)]
pub struct Store {
    repository: Repository,
    state: StateStore,
}

impl Store {
    pub fn new(repository: &Repository) -> Self {
        Self {
            repository: repository.clone(),
            state: repository.store(),
        }
    }

    pub const fn repository(&self) -> &Repository {
        &self.repository
    }

    /// Every family of the selected scope, read under one publication section.
    pub fn snapshot(&self) -> anyhow::Result<ScopeSnapshot> {
        snapshot(&self.state, self.repository.scope())
    }
}

impl std::ops::Deref for Store {
    type Target = StateStore;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

#[cfg(test)]
mod tests;
