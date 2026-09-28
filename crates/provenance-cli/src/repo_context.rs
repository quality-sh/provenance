use crate::store::Store;
use camino::Utf8PathBuf;
use clap::Args;
use provenance_core::ScopeId;
use std::net::{Ipv4Addr, SocketAddr};

#[derive(Clone, Debug, Args)]
pub struct RepoContext {
    #[arg(long, default_value = ".")]
    pub repo: Utf8PathBuf,
    #[arg(long, default_value = "default")]
    pub scope: String,
}

impl RepoContext {
    pub fn new(repo: impl Into<Utf8PathBuf>, scope: impl Into<String>) -> Self {
        Self {
            repo: repo.into(),
            scope: scope.into(),
        }
    }

    pub fn open_graph(&self) -> anyhow::Result<Store> {
        Store::open_required(&self.repo)
    }

    pub fn open_store(&self) -> Store {
        Store::open(&self.repo)
    }

    pub fn scope_id(&self) -> anyhow::Result<ScopeId> {
        ScopeId::new(&self.scope)
    }

    pub fn local_host(&self) -> anyhow::Result<provenance_transport::StatementHost> {
        self.open_graph()?;
        let root = std::fs::canonicalize(&self.repo)?;
        let access = provenance_transport::LocalAccess::new(
            &root,
            "native",
            &self.scope,
            &"0".repeat(64),
            SocketAddr::from((Ipv4Addr::LOCALHOST, 1)),
        )
        .map_err(|failure| anyhow::anyhow!(failure))?;
        Ok(provenance_transport::StatementHost::with_access(
            std::sync::Arc::new(access),
        ))
    }
}
