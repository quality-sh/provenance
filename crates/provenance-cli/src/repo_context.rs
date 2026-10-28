use camino::Utf8PathBuf;
use clap::Args;
use provenance_core::{protocol::failure::OperationFailure, ScopeId};
use provenance_store::repository::{OpenFailure, Repository};

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

    /// Opens the repository once for this command: the initialized-graph gate,
    /// the declared scope and the settings, with the nested-repository hint.
    pub fn open(&self) -> anyhow::Result<Repository> {
        Repository::open(&self.repo, &self.scope)
            .map_err(|error| initialization_hint(&self.repo, open_failure(error)))
    }

    pub fn scope_id(&self) -> anyhow::Result<ScopeId> {
        ScopeId::new(&self.scope)
    }

    pub fn local_host(&self) -> anyhow::Result<provenance_transport::StatementHost> {
        // An undeclared scope is a host refusal, so JSON callers get the refusal object.
        let repository =
            Repository::open(&self.repo, &self.scope).map_err(|error| match error {
                OpenFailure::UnknownScope { .. } => anyhow::anyhow!(OperationFailure::from(error)),
                other => initialization_hint(&self.repo, open_failure(other)),
            })?;
        let repository = provenance_store::operations::LocalRepository::bind(repository, "native")
            .map_err(|failure| anyhow::anyhow!(failure))?;
        Ok(provenance_transport::StatementHost::with_access(
            std::sync::Arc::new(repository),
        ))
    }
}

/// The open failure in the shape the command refusals downcast: a missing
/// graph keeps its marker and a gate failure keeps its cause.
fn open_failure(error: OpenFailure) -> anyhow::Error {
    match error {
        OpenFailure::NotInitialized => provenance_store::layout::GraphNotInitialized.into(),
        OpenFailure::Io(error) => error,
        other => other.into(),
    }
}

/// The manifest scope with the selected id. A scope that the manifest does
/// not declare is refused with the declared scopes, so a mistyped scope never
/// reads as an empty one.
pub fn declared_scope_in<'a>(
    manifest: &'a provenance_core::Manifest,
    selected: &str,
) -> anyhow::Result<&'a provenance_core::Scope> {
    manifest
        .scopes
        .iter()
        .find(|scope| scope.id.as_str() == selected)
        .ok_or_else(|| {
            let declared = manifest
                .scopes
                .iter()
                .map(|scope| scope.id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            anyhow::anyhow!("unknown scope `{selected}`; declared scopes: {declared}")
        })
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
