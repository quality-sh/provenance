use crate::cli::references::{RequirementListCommand, RequirementSingleCommand, SourceListCommand};
use crate::output::OutputFormat;
use camino::Utf8PathBuf;
use clap::Subcommand;

#[derive(Subcommand)]
// `Create` carries every field of a source; the reference verbs carry four flags.
#[allow(clippy::large_enum_variant)]
pub enum SourcesCommand {
    Create {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "policy")]
        source_type: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        reference: Option<String>,
        #[arg(long)]
        commit_pin: Option<String>,
        #[arg(long)]
        effective_date: Option<i64>,
        #[arg(long)]
        review_date: Option<i64>,
        #[arg(long)]
        superseded_by: Option<String>,
        /// An older source this one replaces. Repeat for several.
        #[arg(long)]
        supersedes: Vec<String>,
        #[arg(long)]
        origin_thread: Option<String>,
        #[arg(long)]
        origin_message: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Add or remove an older source this source replaces.
    Supersedes {
        #[command(subcommand)]
        command: SourceListCommand,
    },
}

#[derive(Subcommand)]
pub enum RequirementsCommand {
    Create {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        statement: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long, default_value = "active")]
        status: String,
        #[arg(long)]
        domain_id: Option<String>,
        /// The requirement this one refines.
        #[arg(long)]
        refines: Option<String>,
        /// The resolution this requirement came out of.
        #[arg(long)]
        spawned_by: Option<String>,
        /// A requirement this one depends on. Repeat for several.
        #[arg(long)]
        depends_on: Vec<String>,
        /// An older requirement this one replaces. Repeat for several.
        #[arg(long)]
        supersedes: Vec<String>,
        #[arg(long)]
        origin_thread: Option<String>,
        #[arg(long)]
        origin_message: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    SourceRef {
        #[command(subcommand)]
        command: SourceRefCommand,
    },
    /// Set or clear the requirement this one refines.
    Refines {
        #[command(subcommand)]
        command: RequirementSingleCommand,
    },
    /// Add or remove a requirement this one depends on.
    DependsOn {
        #[command(subcommand)]
        command: RequirementListCommand,
    },
    /// Add or remove an older requirement this one replaces.
    Supersedes {
        #[command(subcommand)]
        command: RequirementListCommand,
    },
    /// Set or clear the resolution this requirement came out of.
    SpawnedBy {
        #[command(subcommand)]
        command: RequirementSingleCommand,
    },
    /// Set, show, or clear the unstructured fog text on a requirement.
    Fog {
        #[command(subcommand)]
        command: FogCommand,
    },
}

#[derive(Subcommand)]
pub enum FogCommand {
    /// Set the fog text: the decisions and investigations sensed but not yet
    /// sharp enough to state as questions.
    Set {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        requirement_id: String,
        #[arg(long)]
        text: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Show the fog text on a requirement.
    Show {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        requirement_id: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Clear the fog text on a requirement.
    Clear {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        requirement_id: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}

#[derive(Subcommand)]
pub enum DomainsCommand {
    Create {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        color: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    List {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}

#[derive(Subcommand)]
pub enum BoundariesCommand {
    Create {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        requirement_id: String,
        #[arg(long)]
        statement: String,
        #[arg(long)]
        source_id: Option<String>,
        #[arg(long)]
        source_clause: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    List {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}

#[derive(Subcommand)]
pub enum SourceRefCommand {
    Add {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        requirement_id: String,
        #[arg(long)]
        source_id: String,
        #[arg(long)]
        clause: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Remove every citation of one source from a requirement.
    Clear {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        requirement_id: String,
        #[arg(long)]
        source_id: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}
