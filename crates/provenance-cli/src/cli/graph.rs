use camino::Utf8PathBuf;
use provenance_cli::repo_context::RepoContext;

/// One `provenance graph-reference` command and its inputs.
pub enum GraphReferenceCommand {
    /// Issue an immutable reference after canonical graph state is committed.
    Issue {
        context: RepoContext,
        /// Git revision to pin. When omitted, clean relevant state at HEAD is required.
        commit: Option<String>,
        correlation_system: Option<String>,
        correlation_key: Option<String>,
    },
    /// Show metadata and graph-family counts from a pinned reference.
    Show {
        repo: Utf8PathBuf,
        reference: Utf8PathBuf,
    },
    /// Verify identity and graph content at the pinned Git revision.
    Verify {
        repo: Utf8PathBuf,
        reference: Utf8PathBuf,
    },
    /// Export the canonical graph reconstructed from the pinned Git revision.
    ExactExport {
        repo: Utf8PathBuf,
        reference: Utf8PathBuf,
    },
}
