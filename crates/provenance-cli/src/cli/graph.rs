use camino::Utf8PathBuf;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum GraphReferenceCommand {
    /// Issue an immutable reference after canonical graph state is committed.
    Issue {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        /// Git revision to pin. When omitted, clean relevant state at HEAD is required.
        #[arg(long)]
        commit: Option<String>,
        #[arg(long, requires = "correlation_key")]
        correlation_system: Option<String>,
        #[arg(long, requires = "correlation_system")]
        correlation_key: Option<String>,
    },
    /// Show metadata and graph-family counts from a pinned reference.
    Show {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        reference: Utf8PathBuf,
    },
    /// Verify identity and graph content at the pinned Git revision.
    Verify {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        reference: Utf8PathBuf,
    },
    /// Export the canonical graph reconstructed from the pinned Git revision.
    ExactExport {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        reference: Utf8PathBuf,
    },
}
