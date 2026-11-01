//! The repository and scope that review links and notify requests act on.

use camino::Utf8PathBuf;
use provenance_store::repository::Repository;
use provenance_transport::local_host::DiscoveredLocalHost;

/// A repository root and the declared scope that one review host serves.
#[derive(Clone, Debug)]
pub struct RepositoryScope {
    pub root: Utf8PathBuf,
    pub scope: String,
}

impl RepositoryScope {
    pub fn new(root: impl Into<Utf8PathBuf>, scope: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            scope: scope.into(),
        }
    }

    /// Opens the repository for one read.
    pub fn open(&self) -> anyhow::Result<Repository> {
        Ok(Repository::open(&self.root, &self.scope)?)
    }

    /// Finds the verified review host of this repository and scope, if one runs.
    pub fn host(&self) -> anyhow::Result<Option<DiscoveredLocalHost>> {
        provenance_transport::local_host::discover(self.root.as_std_path(), &self.scope)
    }
}
