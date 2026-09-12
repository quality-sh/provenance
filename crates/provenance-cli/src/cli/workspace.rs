use crate::output::{JsonFormat, OutputFormat, ReportFormat};
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
pub enum WikiCommand {
    Build {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        /// Defaults to `.provenance/wiki`, which is added to `.gitignore`
        /// automatically. Pass an explicit path to write elsewhere instead
        /// (`.gitignore` is left untouched in that case).
        #[arg(long)]
        out: Option<Utf8PathBuf>,
        /// JSON report written by `coverage scan --format json --output`.
        #[arg(long)]
        coverage: Option<Utf8PathBuf>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Serve {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        /// JSON report written by `coverage scan --format json --output`.
        #[arg(long)]
        coverage: Option<Utf8PathBuf>,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 5175)]
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

#[derive(Subcommand)]
pub enum CoverageCommand {
    Scan {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        path: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        /// JSON report from an earlier scan whose evidence anchors should be
        /// resolved against this scan.
        #[arg(long)]
        baseline: Option<Utf8PathBuf>,
        #[arg(long)]
        validate_rules: bool,
        /// Exit non-zero when the report contains any warnings. The report is
        /// still printed first.
        #[arg(long)]
        strict: bool,
        #[arg(long, value_enum, default_value_t = ReportFormat::Markdown)]
        format: ReportFormat,
        #[arg(long)]
        output: Option<Utf8PathBuf>,
    },
}
