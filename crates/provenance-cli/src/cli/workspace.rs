use crate::output::ReportFormat;
use camino::Utf8PathBuf;

/// One `provenance docs` command and its inputs.
pub enum DocsCommand {
    Check {
        repo: Utf8PathBuf,
    },
    Serve {
        repo: Utf8PathBuf,
        host: String,
        port: u16,
    },
}

/// One `provenance skills` command and its inputs.
pub enum SkillsCommand {
    List,
    Show {
        name: String,
    },
    Install {
        global: bool,
        copy: bool,
        force: bool,
    },
}

/// The `provenance coverage scan` inputs.
pub enum CoverageCommand {
    Scan {
        repo: Utf8PathBuf,
        path: Utf8PathBuf,
        scope: String,
        /// JSON report from an earlier scan whose evidence anchors should be
        /// resolved against this scan.
        baseline: Option<Utf8PathBuf>,
        validate_rules: bool,
        /// Exit non-zero when the report contains any warnings. The report is
        /// still printed first.
        strict: bool,
        format: ReportFormat,
        output: Option<Utf8PathBuf>,
    },
}
