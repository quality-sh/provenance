use crate::output::{JsonFormat, ReportFormat};
use camino::Utf8PathBuf;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum DocsCommand {
    Check {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
    Serve {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 5174)]
        port: u16,
    },
}

#[derive(Subcommand)]
pub enum SkillsCommand {
    List {
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
    Show {
        name: String,
    },
    Install {
        #[arg(long)]
        global: bool,
        #[arg(long)]
        copy: bool,
        #[arg(long)]
        force: bool,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
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
