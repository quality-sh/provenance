use crate::cli::references::{ResolutionListCommand, RuleListCommand};
use crate::output::OutputFormat;
use camino::Utf8PathBuf;
use clap::Subcommand;

#[derive(Subcommand)]
// `Create` carries every field of a resolution; the reference verbs carry four flags.
#[allow(clippy::large_enum_variant)]
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
        /// A requirement this resolution resolves. Repeat for several.
        #[arg(long, required = true)]
        requirement_id: Vec<String>,
        /// An older resolution this one replaces. Repeat for several.
        #[arg(long)]
        supersedes: Vec<String>,
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
    /// Add or remove a requirement this resolution resolves.
    Requirement {
        #[command(subcommand)]
        command: ResolutionListCommand,
    },
    /// Add or remove an older resolution this one replaces.
    Supersedes {
        #[command(subcommand)]
        command: ResolutionListCommand,
    },
}

#[derive(Subcommand)]
// `Create` carries every field of a rule; the read verbs carry three flags.
#[allow(clippy::large_enum_variant)]
pub enum RulesCommand {
    /// Write a new rule into the scope.
    Create {
        /// Repository holding the `.provenance` directory.
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        /// Scope the rule belongs to.
        #[arg(long)]
        scope: String,
        /// Stable id for the rule, used by `#[rule]` and `#[verifies]` sites.
        #[arg(long)]
        id: String,
        /// Short human label. The statement carries the meaning; this is only
        /// what a reader scanning a list sees first.
        #[arg(long)]
        name: Option<String>,
        /// Longer prose about the rule, for anything the statement leaves out.
        #[arg(long)]
        description: Option<String>,
        /// A requirement this rule serves; repeat for several. Each must
        /// already exist in the scope.
        #[arg(long, required = true)]
        requirement_id: Vec<String>,
        /// A resolution that produced this rule; repeat for several. Each
        /// must already exist in the scope.
        #[arg(long)]
        resolution_id: Vec<String>,
        /// What must be true, in one sentence a reader can check code against.
        #[arg(long)]
        statement: String,
        /// One of `active`, `draft`, or `deprecated`.
        #[arg(long, default_value = "active")]
        status: String,
        /// How much a breach costs: `low`, `medium`, `high`, or `critical`.
        #[arg(long, default_value = "medium")]
        severity: String,
        /// Document the rule was read out of, such as an award or a standard.
        #[arg(long)]
        source_document: Option<String>,
        /// Clause or section within that document.
        #[arg(long)]
        source_section: Option<String>,
        /// Shaping thread the rule came out of.
        #[arg(long)]
        origin_thread: Option<String>,
        /// Message within that thread.
        #[arg(long)]
        origin_message: Option<String>,
        /// How to print the created rule.
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// List the rules in a scope, one summary line of record each.
    List {
        /// Repository holding the `.provenance` directory.
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        /// Scope to list.
        #[arg(long, default_value = "default")]
        scope: String,
        /// How to print the list.
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Add or remove a requirement this rule serves.
    Requirement {
        #[command(subcommand)]
        command: RuleListCommand,
    },
    /// Add or remove a resolution that produced this rule.
    Resolution {
        #[command(subcommand)]
        command: RuleListCommand,
    },
    /// Print one rule in full.
    Show {
        /// Repository holding the `.provenance` directory.
        #[arg(long, default_value = ".")]
        repo: Utf8PathBuf,
        /// Scope the rule belongs to.
        #[arg(long, default_value = "default")]
        scope: String,
        /// Id of the rule to print.
        #[arg(long)]
        id: String,
        /// How to print the rule.
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}
