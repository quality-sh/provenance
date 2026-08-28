pub mod dictionary;
pub mod graph;
pub mod ideation;
pub mod knowledge;
pub mod policy;
pub mod sdk;
pub mod shaping;
pub mod workspace;

pub use ideation::{IdeationArtifactKind, SchemaCommand};

use crate::output::OutputFormat;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "provenance", version)]
pub struct Cli {
    /// Drop the advisory notes commands print alongside their output, such as
    /// the warning that this repository has no shaping skills installed.
    #[arg(long, global = true)]
    pub quiet: bool,
    /// Actor ID attested for this run, carried as one claim value into every
    /// mutating operation. Required by repositories whose manifest carries an
    /// rbac section; an attestation, not authentication.
    #[arg(long, global = true, value_name = "ACTOR_ID")]
    pub actor_id: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(name = "__cargo-init", hide = true)]
    CargoInit {
        #[arg(long)]
        package: Option<String>,
    },
    Init {
        #[arg(long)]
        path: Utf8PathBuf,
        #[arg(long)]
        scope: Option<String>,
        #[arg(long, requires = "scope")]
        path_prefix: Option<Utf8PathBuf>,
        /// Repository-local actor ID allowed to attest proposal dispositions.
        /// Repeat for multiple actors. On re-init, omission preserves the allowlist.
        #[arg(long)]
        disposition_actor_id: Vec<String>,
        /// Empty the repository-local disposition actor allowlist.
        #[arg(long, conflicts_with = "disposition_actor_id")]
        clear_disposition_actors: bool,
    },
    Check {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Docs {
        #[command(subcommand)]
        command: workspace::DocsCommand,
    },
    Dictionary {
        #[command(subcommand)]
        command: dictionary::DictionaryCommand,
    },
    Wiki {
        #[command(subcommand)]
        command: workspace::WikiCommand,
    },
    Materialize {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Sources {
        #[command(subcommand)]
        command: knowledge::SourcesCommand,
    },
    Requirements {
        #[command(subcommand)]
        command: knowledge::RequirementsCommand,
    },
    Edges {
        #[command(subcommand)]
        command: graph::EdgesCommand,
    },
    GraphReference {
        #[command(subcommand)]
        command: graph::GraphReferenceCommand,
    },
    Domains {
        #[command(subcommand)]
        command: knowledge::DomainsCommand,
    },
    Boundaries {
        #[command(subcommand)]
        command: knowledge::BoundariesCommand,
    },
    Topics {
        #[command(subcommand)]
        command: shaping::TopicsCommand,
    },
    Questions {
        #[command(subcommand)]
        command: shaping::QuestionsCommand,
    },
    Graph {
        requirement_id: String,
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Resolutions {
        #[command(subcommand)]
        command: policy::ResolutionsCommand,
    },
    Rules {
        #[command(subcommand)]
        command: policy::RulesCommand,
    },
    Traceability {
        rule_id: String,
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Gaps {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Thread {
        #[command(subcommand)]
        command: shaping::ThreadCommand,
    },
    Contributions {
        #[command(subcommand)]
        command: ideation::ContributionsCommand,
    },
    SynthesisPackets {
        #[command(subcommand)]
        command: ideation::SynthesisPacketsCommand,
    },
    Proposals {
        #[command(subcommand)]
        command: ideation::ProposalsCommand,
    },
    Dispositions {
        #[command(subcommand)]
        command: ideation::DispositionsCommand,
    },
    Prime {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Markdown)]
        format: OutputFormat,
        #[arg(long)]
        include_threads: bool,
    },
    Impact {
        id: String,
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long)]
        node_type: String,
        #[arg(long, default_value_t = 3)]
        max_hops: u32,
        #[arg(long)]
        follow_indirect: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Stale {
        /// Older endpoint of the diff range; supply HEAD as the second endpoint.
        base: Option<String>,
        /// Newer endpoint of the diff range.
        head: Option<String>,
        /// Compare this commit with HEAD instead of supplying two endpoints.
        #[arg(long, conflicts_with_all = ["base", "head"])]
        since: Option<String>,
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        /// Exit non-zero when evidence is touched or gone.
        #[arg(long)]
        strict: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Health {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Orphans {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Coverage {
        #[command(subcommand)]
        command: workspace::CoverageCommand,
    },
    /// Typed language façade protocol.
    Sdk {
        #[command(subcommand)]
        command: sdk::SdkCommand,
    },
    SwarmBacktrace {
        #[command(subcommand)]
        command: ideation::SwarmBacktraceCommand,
    },
    Skills {
        #[command(subcommand)]
        command: workspace::SkillsCommand,
    },
    Schema {
        #[command(subcommand)]
        command: ideation::SchemaCommand,
    },
    Validate {
        artifact: ideation::IdeationArtifactKind,
        #[arg(long)]
        input: Utf8PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    Export {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
        #[arg(long)]
        output: Option<Utf8PathBuf>,
    },
    Import {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long)]
        input: Utf8PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    MergeJsonl {
        base: Utf8PathBuf,
        ours: Utf8PathBuf,
        theirs: Utf8PathBuf,
        #[arg(long)]
        output: Option<Utf8PathBuf>,
        /// Repository path the merged result belongs at. Git merge drivers get
        /// three temporary files, so this is how the merge learns which record
        /// type the file holds and which write-time checks to re-apply.
        #[arg(long)]
        path: Option<Utf8PathBuf>,
        /// Actor ID for automatic git invocation: the configured driver
        /// command carries a literal `--actor-id <id>` argument, so the merge
        /// can pass the same policy choke as direct writes. The value is an
        /// attestation configured at clone setup, not authentication.
        #[arg(long)]
        actor_id: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}
