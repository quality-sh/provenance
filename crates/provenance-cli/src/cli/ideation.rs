use crate::output::OutputFormat;
use camino::Utf8PathBuf;
use clap::{Subcommand, ValueEnum};

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
    Assertion,
    Disposition,
    Manifest,
    GraphReference,
    GraphReferenceExport,
}

impl IdeationArtifactKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Contribution => "contribution",
            Self::SynthesisPacket => "synthesis-packet",
            Self::Proposal => "proposal",
            Self::Assertion => "assertion",
            Self::Disposition => "disposition",
            Self::Manifest => "manifest",
            Self::GraphReference => "graph-reference",
            Self::GraphReferenceExport => "graph-reference-export",
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
        /// Create the proposal and its assertion atomically.
        #[arg(long, requires = "synthesis_packet_id")]
        assertion_id: Option<String>,
        /// Synthesis packet used by the atomic assertion.
        #[arg(long, requires = "assertion_id")]
        synthesis_packet_id: Option<String>,
        /// Immutable assertion ID this proposal builds on. Repeatable.
        #[arg(long)]
        builds_on: Vec<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    Assert {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        proposal_id: String,
        #[arg(long)]
        synthesis_packet_id: String,
        #[arg(long)]
        supporting_claim_id: Vec<String>,
        /// Atomically clear resolved blocking human decisions before asserting.
        #[arg(long, requires = "decision_key")]
        resolve_human_gate: bool,
        /// Blocking human decision key that was resolved. Repeatable.
        #[arg(long, requires = "resolve_human_gate")]
        decision_key: Vec<String>,
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
    /// Surface undisposed proposals when current work enters their territory.
    Surface {
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        #[arg(long)]
        scope: String,
        /// Repository-relative path touched by the current diff. Repeat for multiple paths.
        #[arg(long)]
        changed_path: Vec<String>,
        /// Explicit existing territory type, paired with --target-id.
        #[arg(long)]
        target_type: Option<String>,
        /// Explicit existing territory ID, paired with --target-type.
        #[arg(long)]
        target_id: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum DispositionsCommand {
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
        /// External system owning the correlated action (for example, github).
        #[arg(long, requires_all = ["external_scope", "external_kind", "external_key"])]
        external_system: Option<String>,
        /// Namespace within the external system (for example, owner/repository).
        #[arg(long, requires_all = ["external_system", "external_kind", "external_key"])]
        external_scope: Option<String>,
        /// Generic external action type (for example, issue, commit, ticket, or deployment).
        #[arg(long, requires_all = ["external_system", "external_scope", "external_key"])]
        external_kind: Option<String>,
        /// Stable action key within the external system, scope, and kind.
        #[arg(long, requires_all = ["external_system", "external_scope", "external_kind"])]
        external_key: Option<String>,
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
