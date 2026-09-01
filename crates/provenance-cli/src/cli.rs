use crate::output::OutputFormat;
use camino::Utf8PathBuf;
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;

#[derive(Parser)]
pub struct Cli {
    #[arg(long, global = true)]
    pub quiet: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Init {
        #[arg(long)]
        path: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long, default_value = ".")]
        path_prefix: Utf8PathBuf,
    },
    Check {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Docs {
        #[command(subcommand)]
        command: DocsCommand,
    },
    Wiki {
        #[command(subcommand)]
        command: WikiCommand,
    },
    Materialize {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Sources {
        #[command(subcommand)]
        command: SourcesCommand,
    },
    Requirements {
        #[command(subcommand)]
        command: RequirementsCommand,
    },
    Edges {
        #[command(subcommand)]
        command: EdgesCommand,
    },
    Domains {
        #[command(subcommand)]
        command: DomainsCommand,
    },
    Boundaries {
        #[command(subcommand)]
        command: BoundariesCommand,
    },
    Topics {
        #[command(subcommand)]
        command: TopicsCommand,
    },
    Questions {
        #[command(subcommand)]
        command: QuestionsCommand,
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
        command: ResolutionsCommand,
    },
    Rules {
        #[command(subcommand)]
        command: RulesCommand,
    },
    Services {
        #[command(subcommand)]
        command: ServicesCommand,
    },
    ServiceBindings {
        #[command(subcommand)]
        command: ServiceBindingsCommand,
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
        command: ThreadCommand,
    },
    Contributions {
        #[command(subcommand)]
        command: ContributionsCommand,
    },
    SynthesisPackets {
        #[command(subcommand)]
        command: SynthesisPacketsCommand,
    },
    Proposals {
        #[command(subcommand)]
        command: ProposalsCommand,
    },
    PromotionDecisions {
        #[command(subcommand)]
        command: PromotionDecisionsCommand,
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
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long, default_value_t = 0)]
        min_age_days: u32,
        #[arg(long)]
        rule_severities: Option<String>,
        #[arg(long, default_value_t = 0)]
        min_downstream_rules: u32,
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
        command: CoverageCommand,
    },
    SwarmBacktrace {
        #[command(subcommand)]
        command: SwarmBacktraceCommand,
    },
    Skills {
        #[command(subcommand)]
        command: SkillsCommand,
    },
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },
    Validate {
        artifact: IdeationArtifactKind,
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
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
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
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Aggregate the spool; optionally join session metadata supplied by a
    /// sister system via the provenance-dogfood-enrichment/v1 contract.
    Report {
        /// Enrichment JSON conforming to provenance-dogfood-enrichment/v1
        /// (a file path, or "-" for stdin).
        #[arg(long)]
        enrich: Option<Utf8PathBuf>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}

#[cfg(feature = "dogfood")]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Serialize, serde::Deserialize,
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
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum DogfoodSeverity {
    Blocked,
    Workaround,
    Annoyance,
}

#[derive(Subcommand)]
pub enum SwarmBacktraceCommand {
    Land {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long)]
        run_dir: Utf8PathBuf,
        #[arg(long)]
        replace: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum IdeationArtifactKind {
    Contribution,
    SynthesisPacket,
    Proposal,
}

impl IdeationArtifactKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Contribution => "contribution",
            Self::SynthesisPacket => "synthesis-packet",
            Self::Proposal => "proposal",
        }
    }
}

#[derive(Clone, Copy, Subcommand)]
pub enum SchemaCommand {
    Show {
        artifact: IdeationArtifactKind,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
}

#[derive(Subcommand)]
pub enum DocsCommand {
    Check {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
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
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Serve {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 5175)]
        port: u16,
    },
}

#[derive(Subcommand)]
pub enum EdgesCommand {
    Create {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long = "type")]
        edge_type: String,
        #[arg(long)]
        from_type: String,
        #[arg(long)]
        from_id: String,
        #[arg(long)]
        to_type: String,
        #[arg(long)]
        to_id: String,
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
    Delete {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}

#[derive(Subcommand)]
pub enum SkillsCommand {
    List {
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
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
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum ContributionsCommand {
    Create {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        target_type: String,
        #[arg(long)]
        target_id: String,
        #[arg(long)]
        participant_slot: String,
        #[arg(long)]
        stance: String,
        #[arg(long)]
        strongest_finding: String,
        #[arg(long, default_value = "[]")]
        evidence_json: String,
        #[arg(long, default_value = "[]")]
        claims_json: String,
        #[arg(long, default_value = "[]")]
        risks_json: String,
        #[arg(long, default_value = "[]")]
        objections_json: String,
        #[arg(long, default_value = "[]")]
        challenges_json: String,
        #[arg(long, default_value = "[]")]
        suggested_changes_json: String,
        #[arg(long, default_value = "[]")]
        unsupported_recommendations_json: String,
        #[arg(long, default_value = "medium")]
        uncertainty_level: String,
        #[arg(long)]
        uncertainty_rationale: String,
        #[arg(long, default_value = "[]")]
        open_questions_json: String,
        #[arg(long)]
        replace: bool,
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
#[allow(clippy::large_enum_variant)]
pub enum SynthesisPacketsCommand {
    Create {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        target_type: String,
        #[arg(long)]
        target_id: String,
        #[arg(long)]
        summary: String,
        #[arg(long, default_value = "[]")]
        consensus_json: String,
        #[arg(long, default_value = "[]")]
        contested_claims_json: String,
        #[arg(long, default_value = "[]")]
        minority_objections_json: String,
        #[arg(long, default_value = "[]")]
        evidence_gaps_json: String,
        #[arg(long, default_value = "[]")]
        unsupported_speculation_json: String,
        #[arg(long, default_value = "[]")]
        open_questions_json: String,
        #[arg(long, default_value = "[]")]
        suggested_artifacts_json: String,
        #[arg(long, default_value = "[]")]
        required_human_decisions_json: String,
        #[arg(long)]
        replace: bool,
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
#[allow(clippy::large_enum_variant)]
pub enum ProposalsCommand {
    Create {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        proposal_key: String,
        #[arg(long)]
        proposal_type: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        summary: String,
        #[arg(long)]
        confidence: Option<f64>,
        #[arg(long)]
        target_type: String,
        #[arg(long)]
        target_id: String,
        #[arg(long)]
        source_id: Vec<String>,
        #[arg(long, default_value = "[]")]
        evidence_json: String,
        #[arg(long)]
        supporting_claim_id: Vec<String>,
        #[arg(long, default_value = "proposed")]
        promotion_state: String,
        #[arg(long)]
        duplicate_of: Option<String>,
        #[arg(long)]
        superseded_by: Option<String>,
        #[arg(long)]
        replace: bool,
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
#[allow(clippy::large_enum_variant)]
pub enum PromotionDecisionsCommand {
    Create {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        proposal_id: String,
        #[arg(long)]
        decision: String,
        #[arg(long)]
        rationale: String,
        #[arg(long)]
        actor_id: String,
        #[arg(long)]
        actor_type: String,
        #[arg(long)]
        actor_name: Option<String>,
        #[arg(long)]
        canonical_artifact_type: Option<String>,
        #[arg(long)]
        canonical_artifact_id: Option<String>,
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
pub enum CoverageCommand {
    Scan {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        path: Utf8PathBuf,
        #[arg(long, default_value = "default")]
        scope: String,
        #[arg(long)]
        validate_rules: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
        #[arg(long)]
        output: Option<Utf8PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum ThreadCommand {
    Post {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        parent_type: String,
        #[arg(long)]
        parent_id: String,
        #[arg(long)]
        role: String,
        body: String,
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
pub enum ResolutionsCommand {
    Create {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        requirement_id: Option<String>,
        #[arg(long)]
        position: String,
        #[arg(long)]
        rationale: String,
        #[arg(long, default_value = "proposed")]
        status: String,
        #[arg(long)]
        context: Option<String>,
        #[arg(long)]
        enforcement: Option<String>,
        #[arg(long)]
        confidence: Option<f64>,
        #[arg(long = "input-type")]
        input_type: Vec<String>,
        #[arg(long = "input-reference")]
        input_reference: Vec<String>,
        #[arg(long = "input-summary")]
        input_summary: Vec<String>,
        #[arg(long)]
        made_by: Option<String>,
        #[arg(long)]
        approved_by: Option<String>,
        #[arg(long)]
        approved_at: Option<i64>,
        #[arg(long)]
        superseded_by: Option<String>,
        #[arg(long)]
        origin_thread: Option<String>,
        #[arg(long)]
        origin_message: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}

#[derive(Subcommand)]
pub enum RulesCommand {
    Create {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        rule_code: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        requirement_id: Option<String>,
        #[arg(long)]
        resolution_id: Option<String>,
        #[arg(long)]
        statement: String,
        #[arg(long, default_value = "active")]
        status: String,
        #[arg(long, default_value = "medium")]
        severity: String,
        #[arg(long)]
        rule_type: Option<String>,
        #[arg(long)]
        modality: Option<String>,
        #[arg(long)]
        confidence: Option<f64>,
        #[arg(long)]
        extraction_method: Option<String>,
        #[arg(long)]
        source_document: Option<String>,
        #[arg(long)]
        source_section: Option<String>,
        #[arg(long)]
        origin_thread: Option<String>,
        #[arg(long)]
        origin_message: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}

#[derive(Args)]
pub struct ServiceCreateArgs {
    #[arg(long, default_value = ".")]
    pub repo: Utf8PathBuf,
    #[arg(long)]
    pub scope: String,
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long)]
    pub owner: Option<String>,
    #[arg(long)]
    pub repository: Option<String>,
    #[arg(long)]
    pub environment: Option<String>,
    #[arg(long)]
    pub tier: Option<String>,
    #[arg(long)]
    pub external_id: Option<String>,
    #[arg(long, default_value = "active")]
    pub status: String,
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Subcommand)]
pub enum ServicesCommand {
    Create(Box<ServiceCreateArgs>),
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
pub enum ServiceBindingsCommand {
    Create {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        rule_id: String,
        #[arg(long)]
        service_id: String,
        #[arg(long)]
        binding_type: String,
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
        #[arg(long)]
        origin_thread: Option<String>,
        #[arg(long)]
        origin_message: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
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
pub enum TopicsCommand {
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
        title: String,
        #[arg(long, default_value = "open")]
        status: String,
        #[arg(long, default_value = "[]")]
        links_json: String,
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
    /// Claim a topic so concurrent sessions skip it. Claiming an
    /// already-claimed topic is an error showing who holds it.
    Claim {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        /// Actor name recorded on the claim.
        #[arg(long)]
        actor: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Release a claimed topic without closing it.
    Release {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Close a topic. Closing clears any claim on it.
    Close {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}

#[derive(Subcommand)]
pub enum QuestionsCommand {
    /// Create a question. A question should be resolvable in one agent
    /// session; otherwise it is fog or needs decomposition.
    Create {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        topic_id: String,
        #[arg(long)]
        question: String,
        /// Resolution method: grill, prototype, research, verify, or task.
        #[arg(long)]
        method: String,
        #[arg(long, default_value = "open")]
        status: String,
        #[arg(long)]
        answer: Option<String>,
        #[arg(long, default_value = "[]")]
        links_json: String,
        #[arg(long)]
        resolution_id: Option<String>,
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
    /// Update mutable question state after creation.
    Update {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        /// Resolution method: grill, prototype, research, verify, or task.
        #[arg(long)]
        method: Option<String>,
        /// Status: open, `blocked_on_human`, or answered. Hyphens are accepted.
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        links_json: Option<String>,
        #[arg(long)]
        resolution_id: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Claim a question so concurrent sessions skip it. Claiming an
    /// already-claimed question is an error showing who holds it.
    Claim {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        /// Actor name recorded on the claim.
        #[arg(long)]
        actor: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Release a claimed question without answering it.
    Release {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Record the answer to a question. Answering clears any claim on it.
    Answer {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        answer: String,
        #[arg(long)]
        resolution_id: Option<String>,
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
}

#[derive(Serialize)]
pub struct Status<'a> {
    pub status: &'a str,
}
