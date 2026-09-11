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
}
