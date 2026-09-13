use crate::output::OutputFormat;
use camino::Utf8PathBuf;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum ReportCommand {
    /// Render the deterministic pull request report from a report envelope.
    Render {
        /// Report envelope JSON (schema version 1).
        #[arg(long)]
        input: Utf8PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Markdown)]
        format: OutputFormat,
        /// Write the result to this file instead of standard output.
        #[arg(long)]
        output: Option<Utf8PathBuf>,
    },
    /// Build a report envelope from a real repository at a named base and
    /// head.
    Build {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        /// Older endpoint of the comparison range.
        #[arg(long)]
        base: String,
        /// Newer endpoint of the comparison range.
        #[arg(long)]
        head: String,
        /// Repository identity in `owner/name` form for report links.
        #[arg(long)]
        repository: String,
        #[arg(long, default_value = "default")]
        scope: String,
        /// Scan this path instead of the repository root; a partial scan is
        /// recorded as incomplete.
        #[arg(long)]
        path: Option<Utf8PathBuf>,
        /// Write the envelope JSON to this file instead of standard output.
        #[arg(long)]
        output: Option<Utf8PathBuf>,
    },
}
