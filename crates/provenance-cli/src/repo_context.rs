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
        Store::open_required(&self.repo).map_err(|error| initialization_hint(&self.repo, error))
    }

    /// The manifest scope the caller selected, with the manifest. Refuses
    /// before any read when the repository is not initialized or when the
    /// manifest does not declare the scope, so a mistyped scope never reads
    /// as an empty one.
    #[provenance_macros::rule("rule_coverage_refuses_undeclared_scope")]
    pub fn declared_scope(
        &self,
    ) -> anyhow::Result<(provenance_core::Scope, provenance_core::Manifest)> {
        let store = self.open_graph()?;
        let selected = self.scope.as_str();
        provenance_store::layout::with_initialized_graph(store.layout(), |manifest| {
            let scope = manifest
                .scopes
                .iter()
                .find(|scope| scope.id.as_str() == selected)
                .cloned()
                .ok_or_else(|| unknown_scope(selected, &manifest))?;
            Ok((scope, manifest))
        })
        .map_err(|error| initialization_hint(&self.repo, error))
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

/// The refusal for a scope the manifest does not declare.
pub fn unknown_scope(scope: &str, manifest: &provenance_core::Manifest) -> anyhow::Error {
    let declared = manifest
        .scopes
        .iter()
        .map(|scope| scope.id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    anyhow::anyhow!("unknown scope `{scope}`; declared scopes: {declared}")
}

/// A command ran inside a directory of an initialized repository instead of
/// at its root. `provenance init` here would nest a second graph, so the
/// refusal names the root to pass as `--repo`.
#[derive(Debug)]
pub struct NestedRepository {
    pub here: Utf8PathBuf,
    pub root: Utf8PathBuf,
}

impl std::fmt::Display for NestedRepository {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Provenance graph is not initialized in {}; the enclosing repository {} is \
             initialized; run the command with --repo {}",
            self.here, self.root, self.root
        )
    }
}

impl std::error::Error for NestedRepository {}

/// Whether an error says the graph is not initialized, with or without the
/// enclosing-repository hint.
pub fn is_not_initialized(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<provenance_store::layout::GraphNotInitialized>()
        .is_some()
        || error.downcast_ref::<NestedRepository>().is_some()
}

/// Replaces a not-initialized refusal with the nested-repository refusal
/// when an ancestor directory holds an initialized graph.
#[provenance_macros::rule("rule_command_names_enclosing_repository")]
pub fn initialization_hint(repo: &camino::Utf8Path, error: anyhow::Error) -> anyhow::Error {
    if error
        .downcast_ref::<provenance_store::layout::GraphNotInitialized>()
        .is_none()
    {
        return error;
    }
    let Some(root) = enclosing_repository(repo) else {
        return error;
    };
    NestedRepository {
        here: repo.to_path_buf(),
        root,
    }
    .into()
}

fn enclosing_repository(repo: &camino::Utf8Path) -> Option<Utf8PathBuf> {
    let here = std::fs::canonicalize(repo.as_std_path()).ok()?;
    let here = Utf8PathBuf::from_path_buf(here).ok()?;
    here.ancestors()
        .skip(1)
        .find(|ancestor| {
            ancestor
                .join(".provenance/state/manifest.json")
                .as_std_path()
                .is_file()
        })
        .map(camino::Utf8Path::to_path_buf)
}
