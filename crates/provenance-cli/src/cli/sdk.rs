use crate::output::OutputFormat;
use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

/// What every structured query primitive needs besides its stdin request.
#[derive(Args)]
pub struct QueryArgs {
    #[arg(long)]
    pub repo: Option<Utf8PathBuf>,
    #[arg(long, default_value = "default")]
    pub scope: String,
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    pub format: OutputFormat,
}

#[derive(Subcommand)]
pub enum SdkCommand {
    /// Check one unfinished Requirement or Rule statement from stdin.
    CheckStatement {
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Report engine compatibility and the resolved project root.
    Info {
        #[arg(long)]
        repo: Option<Utf8PathBuf>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Preview one desired-state reconciliation without writing it.
    Plan {
        #[arg(long)]
        repo: Option<Utf8PathBuf>,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Reconcile one desired-state document read from stdin.
    Apply {
        #[arg(long)]
        repo: Option<Utf8PathBuf>,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Start one callback-backed verification run described on stdin.
    BeginVerification {
        #[arg(long)]
        repo: Option<Utf8PathBuf>,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Finish one callback-backed verification run described on stdin.
    CompleteVerification {
        #[arg(long)]
        repo: Option<Utf8PathBuf>,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// List cached callback-backed verification evidence.
    VerificationRuns {
        #[arg(long)]
        repo: Option<Utf8PathBuf>,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long)]
        rule: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Read one canonical record named by a JSON request on stdin.
    Get {
        #[command(flatten)]
        query: QueryArgs,
    },
    /// Find canonical records whose text contains a phrase.
    Search {
        #[command(flatten)]
        query: QueryArgs,
    },
    /// Read the canonical records one relation away from a record.
    Neighbors {
        #[command(flatten)]
        query: QueryArgs,
    },
    /// Walk outward from a record for a bounded number of hops.
    Trace {
        #[command(flatten)]
        query: QueryArgs,
    },
    /// Read the Rules a record reaches and the code standing behind them.
    Impact {
        #[command(flatten)]
        query: QueryArgs,
    },
    /// Read the bindings, runs, review, and stale report for one Rule.
    Evidence {
        #[command(flatten)]
        query: QueryArgs,
    },
    /// Read which evidence sites a commit range disturbed.
    Stale {
        #[command(flatten)]
        query: QueryArgs,
    },
    /// Read the Rules bound to one code site.
    ResolveSymbol {
        #[command(flatten)]
        query: QueryArgs,
    },
    /// List durable typed verification relationships.
    VerificationBindings {
        #[arg(long)]
        repo: Option<Utf8PathBuf>,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long)]
        rule: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}
