use crate::output::ReportFormat;
use camino::Utf8PathBuf;

/// One `provenance report` command and its inputs.
pub enum ReportCommand {
    /// Render the deterministic pull request report from a report envelope.
    Render {
        /// Report envelope JSON (schema version 1).
        input: Utf8PathBuf,
        format: ReportFormat,
        /// Write the result to this file instead of standard output.
        output: Option<Utf8PathBuf>,
    },
    /// Build a report envelope from a real repository at a named base and
    /// head.
    Build {
        repo: Utf8PathBuf,
        /// Older endpoint of the comparison range.
        base: String,
        /// Newer endpoint of the comparison range.
        head: String,
        /// Repository identity in `owner/name` form for report links.
        repository: String,
        scope: String,
        /// Scan this path instead of the repository root; a partial scan is
        /// recorded as incomplete.
        path: Option<Utf8PathBuf>,
        /// Write the envelope JSON to this file instead of standard output.
        output: Option<Utf8PathBuf>,
    },
}
