pub mod dictionary;
pub mod graph;
pub mod ideation;
pub mod report;
pub mod workspace;

pub use ideation::{IdeationArtifactKind, SchemaCommand};

use crate::output::JsonFormat;
#[cfg(feature = "dogfood")]
use camino::Utf8PathBuf;
#[cfg(feature = "dogfood")]
use clap::ValueEnum;
use clap::{Parser, Subcommand};
use provenance_cli::repo_context::RepoContext;

/// How the caller runs provenance, so init prints commands that the caller can run.
#[derive(Clone, Copy, Debug, Default)]
pub enum InvocationChannel {
    #[default]
    Native,
    Typescript,
}

impl InvocationChannel {
    /// The channel that one `--invocation-channel` value names.
    pub fn from_flag(value: &str) -> Option<Self> {
        match value {
            "native" => Some(Self::Native),
            "typescript" => Some(Self::Typescript),
            _ => None,
        }
    }
}

/// The package manager that runs the TypeScript package.
#[derive(Clone, Copy, Debug)]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
    Deno,
    Nub,
}

impl PackageManager {
    /// The package manager that one `--package-manager` value names.
    pub fn from_flag(value: &str) -> Option<Self> {
        Some(match value {
            "npm" => Self::Npm,
            "pnpm" => Self::Pnpm,
            "yarn" => Self::Yarn,
            "bun" => Self::Bun,
            "deno" => Self::Deno,
            "nub" => Self::Nub,
            _ => return None,
        })
    }
}

#[derive(Parser)]
#[command(
    name = "provenance",
    version,
    after_help = concat!(
        "Dynamic commands:\n",
        "  provenance api [PATH]\n",
        "  provenance api --filter <text>\n",
        "  provenance <id> get --view children --depth <N>\n\n",
        "Discussion actions: provenance discussions [<discussion-id> get], or provenance ",
        "<record-id> discussions|discuss, or provenance <discussion-id> reply."
    )
)]
pub struct Cli {
    /// Drop the advisory notes commands print alongside their output, such as
    /// the warning that this repository has no shaping skills installed.
    #[arg(long, global = true)]
    pub quiet: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Docs {
        #[command(subcommand)]
        command: workspace::DocsCommand,
    },
    /// Serve local review assets and authorized repository operations.
    Review(crate::review::Options),
    GraphReference {
        #[command(subcommand)]
        command: graph::GraphReferenceCommand,
    },
    Graph {
        requirement_id: String,
        #[command(flatten)]
        context: RepoContext,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
    Traceability {
        rule_id: String,
        #[command(flatten)]
        context: RepoContext,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
    Gaps {
        #[command(flatten)]
        context: RepoContext,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
    Health {
        #[command(flatten)]
        context: RepoContext,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
    Orphans {
        #[command(flatten)]
        context: RepoContext,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
    Coverage {
        #[command(subcommand)]
        command: workspace::CoverageCommand,
    },
    /// Deterministic report rendering from a versioned report envelope.
    Report {
        #[command(subcommand)]
        command: report::ReportCommand,
    },
    SwarmBacktrace {
        #[command(subcommand)]
        command: ideation::SwarmBacktraceCommand,
    },
    Skills {
        #[command(subcommand)]
        command: workspace::SkillsCommand,
    },
    /// Dev-build-only: record pain-point notes about provenance itself.
    #[cfg(feature = "dogfood")]
    Dogfood {
        #[command(subcommand)]
        command: DogfoodCommand,
    },
}

/// Dev-build-only agent feedback about provenance itself. Notes are appended
/// to a local spool; nothing ever leaves the machine.
#[cfg(feature = "dogfood")]
#[derive(Subcommand)]
pub enum DogfoodCommand {
    /// Record one pain-point note. Cheap by design: three enums and a sentence.
    Note {
        /// The part of provenance the note concerns: a subcommand name, or "general".
        #[arg(long)]
        surface: String,
        #[arg(long, value_enum)]
        category: DogfoodCategory,
        /// Impact the pain had on the task at hand.
        #[arg(long, value_enum)]
        severity: DogfoodSeverity,
        /// What you were trying to do, what happened, what you expected.
        #[arg(long)]
        detail: Option<String>,
        #[arg(long)]
        suggestion: Option<String>,
        /// One-line summary of the pain point.
        summary: String,
    },
    /// Print the local note spool.
    List {
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
    /// Aggregate the spool; optionally join session metadata supplied by a
    /// sister system via the provenance-dogfood-enrichment/v1 contract.
    Report {
        /// Enrichment JSON conforming to provenance-dogfood-enrichment/v1
        /// (a file path, or "-" for stdin).
        #[arg(long)]
        enrich: Option<Utf8PathBuf>,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
    /// Dev-build-only triage of captured notes: mark them handled or reopen
    /// them. State lives beside the spool; the capture history is never
    /// rewritten.
    Triage {
        #[command(subcommand)]
        command: TriageCommand,
    },
    /// Dev-build-only: try the whole first-run experience in a throwaway
    /// sandbox. Serves the STE dictionary from loopback, records no note,
    /// and touches nothing outside the sandbox.
    Simulate {
        /// Keep the sandbox directory after the run for inspection.
        #[arg(long)]
        keep: bool,
        /// Create the sandbox inside this existing directory instead of the
        /// system temp location.
        #[arg(long)]
        dir: Option<Utf8PathBuf>,
    },
}

/// Dev-build-only triage state for captured notes. Every state change is an
/// append to a sibling state file, so the note spool stays untouched.
#[cfg(feature = "dogfood")]
#[derive(Subcommand)]
pub enum TriageCommand {
    /// Mark one note handled.
    Handle {
        /// Note id, or an unambiguous prefix of one, as shown by
        /// `provenance dogfood triage list`.
        id: String,
        /// Why the note is handled: a short reason or an issue reference.
        #[arg(long)]
        reason: Option<String>,
    },
    /// Return a handled note to the unhandled state.
    Reopen {
        /// Note id, or an unambiguous prefix of one, as shown by
        /// `provenance dogfood triage list`.
        id: String,
    },
    /// Show every captured note with its identifier and triage state.
    List {
        /// Only show notes in this triage state.
        #[arg(long, value_enum, default_value_t = TriageFilter::All)]
        status: TriageFilter,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
}

/// Which triage state a `dogfood triage list` view shows.
#[cfg(feature = "dogfood")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TriageFilter {
    Unhandled,
    Handled,
    All,
}

#[cfg(feature = "dogfood")]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    ValueEnum,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum DogfoodCategory {
    Friction,
    Confusion,
    Missing,
    Bug,
    Idea,
}

#[cfg(feature = "dogfood")]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    ValueEnum,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum DogfoodSeverity {
    Blocked,
    Workaround,
    Annoyance,
}
