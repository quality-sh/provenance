//! Opens a local repository for one command, and the host that serves its porcelain.

use crate::check::RepositoryCheckPort;
use crate::StatementHost;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{protocol::failure::OperationFailure, ScopeId};
use provenance_store::repository::{OpenFailure, Repository};
use std::sync::Arc;

/// Opens the repository at `repo` for `scope` and returns the host with its check adapter
/// and its repository files.
///
/// An undeclared scope is a host refusal, so JSON callers get the refusal object. A
/// directory inside an initialized repository gets the nested-repository refusal.
pub fn open(repo: &Utf8Path, scope: &str) -> anyhow::Result<StatementHost> {
    let repository = Repository::open(repo, scope).map_err(|error| match error {
        OpenFailure::UnknownScope { .. } => anyhow::anyhow!(OperationFailure::from(error)),
        other => initialization_hint(repo, open_failure(other)),
    })?;
    let repository = provenance_store::operations::LocalRepository::bind(repository, "native")
        .map_err(|failure| anyhow::anyhow!(failure))?;
    let check = RepositoryCheckPort::new(working_tree(repo), false, None);
    Ok(StatementHost::with_access(Arc::new(repository))
        .with_check_port(Arc::new(check))
        .with_repository(RepoContext::new(repo, scope)))
}

/// The repository root as UTF-8. The repository refuses a root that is not UTF-8 first,
/// so its open error is the one to report.
pub fn utf8_root(path: std::path::PathBuf, scope: &str) -> anyhow::Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path).map_err(|path| match Repository::open(&path, scope) {
        Err(error) => anyhow::Error::new(error),
        Ok(_) => anyhow::anyhow!("repository path {} is not UTF-8", path.display()),
    })
}

/// The canonical working tree, which the opened repository also reads.
pub fn working_tree(repo: &Utf8Path) -> Utf8PathBuf {
    dunce::canonicalize(repo)
        .ok()
        .and_then(|path| Utf8PathBuf::from_path_buf(path).ok())
        .unwrap_or_else(|| repo.to_path_buf())
}

/// The repository root and scope that one command opens.
#[derive(Clone, Debug)]
pub struct RepoContext {
    pub repo: Utf8PathBuf,
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

    /// The host of this repository and scope, with its check adapter.
    pub fn host(&self) -> anyhow::Result<StatementHost> {
        open(&self.repo, &self.scope)
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
pub fn initialization_hint(repo: &Utf8Path, error: anyhow::Error) -> anyhow::Error {
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

fn enclosing_repository(repo: &Utf8Path) -> Option<Utf8PathBuf> {
    let here = dunce::canonicalize(repo.as_std_path()).ok()?;
    let here = Utf8PathBuf::from_path_buf(here).ok()?;
    here.ancestors()
        .skip(1)
        .find(|ancestor| {
            ancestor
                .join(".provenance")
                .join("state")
                .join("manifest.json")
                .as_std_path()
                .is_file()
        })
        .map(Utf8Path::to_path_buf)
}

/// The record kinds of `target` in `scope`, read under the read-only marker: the read
/// takes no publication lock and runs no recovery, so it never waits for another
/// command and never writes to the repository. While a publication runs, the answer
/// is best-effort.
pub fn read_only_record_kinds(
    repo: &Utf8Path,
    scope: &ScopeId,
    target: &str,
) -> anyhow::Result<Vec<provenance_core::NodeType>> {
    let layout = provenance_store::layout::ProvenanceLayout::new(repo);
    provenance_store::publication::read_only_record_kinds(&layout, scope, target)
}

/// The warning that the repository has no dictionary index, when it has none.
pub fn missing_dictionary_warning(repo: &Utf8Path) -> Option<String> {
    let layout = provenance_store::layout::ProvenanceLayout::new(repo);
    provenance_store::dictionary_reference::missing_index_warning(&layout)
}

/// Whether the initialized repository at `repo` has a reviewer: one disposition actor
/// or more.
pub fn has_reviewer(repo: &Utf8Path) -> anyhow::Result<bool> {
    let layout = provenance_store::layout::ProvenanceLayout::new(repo);
    provenance_store::layout::with_initialized_graph(&layout, |manifest| {
        Ok(!manifest.disposition_actor_ids.is_empty())
    })
}

/// The per-user cache directory of this machine.
pub fn user_cache_directory() -> Option<std::path::PathBuf> {
    provenance_store::user_cache::cache_directory()
}
