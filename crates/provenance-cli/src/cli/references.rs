//! The add, set, and clear commands for reference fields. The owner flag
//! names the owner kind; `--target-id` names the record the field points at.

use crate::output::OutputFormat;
use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct RequirementTarget {
    #[arg(long, default_value = ".")]
    pub repo: Utf8PathBuf,
    #[arg(long)]
    pub scope: String,
    #[arg(long)]
    pub requirement_id: String,
    #[arg(long)]
    pub target_id: String,
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Args)]
pub struct RequirementOnly {
    #[arg(long, default_value = ".")]
    pub repo: Utf8PathBuf,
    #[arg(long)]
    pub scope: String,
    #[arg(long)]
    pub requirement_id: String,
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Args)]
pub struct RuleTarget {
    #[arg(long, default_value = ".")]
    pub repo: Utf8PathBuf,
    #[arg(long)]
    pub scope: String,
    #[arg(long)]
    pub rule_id: String,
    #[arg(long)]
    pub target_id: String,
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Args)]
pub struct ResolutionTarget {
    #[arg(long, default_value = ".")]
    pub repo: Utf8PathBuf,
    #[arg(long)]
    pub scope: String,
    #[arg(long)]
    pub resolution_id: String,
    #[arg(long)]
    pub target_id: String,
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Args)]
pub struct SourceTarget {
    #[arg(long, default_value = ".")]
    pub repo: Utf8PathBuf,
    #[arg(long)]
    pub scope: String,
    #[arg(long)]
    pub source_id: String,
    #[arg(long)]
    pub target_id: String,
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Args)]
pub struct QuestionTarget {
    #[arg(long, default_value = ".")]
    pub repo: Utf8PathBuf,
    #[arg(long)]
    pub scope: String,
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub target_id: String,
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Args)]
pub struct QuestionOnly {
    #[arg(long, default_value = ".")]
    pub repo: Utf8PathBuf,
    #[arg(long)]
    pub scope: String,
    #[arg(long)]
    pub id: String,
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// `requirements refines` and `requirements spawned-by`: one target at most.
#[derive(Subcommand)]
pub enum RequirementSingleCommand {
    /// Point the field at a record, replacing the current target.
    Set(RequirementTarget),
    /// Empty the field.
    Clear(RequirementOnly),
}

/// `requirements depends-on` and `requirements supersedes`: a list.
#[derive(Subcommand)]
pub enum RequirementListCommand {
    /// Add one record to the list.
    Add(RequirementTarget),
    /// Remove one record from the list.
    Clear(RequirementTarget),
}

/// `rules requirement` and `rules resolution`.
#[derive(Subcommand)]
pub enum RuleListCommand {
    /// Add one record to the list.
    Add(RuleTarget),
    /// Remove one record from the list; a rule keeps its last requirement.
    Clear(RuleTarget),
}

/// `resolutions requirement` and `resolutions supersedes`.
#[derive(Subcommand)]
pub enum ResolutionListCommand {
    /// Add one record to the list.
    Add(ResolutionTarget),
    /// Remove one record from the list; a resolution keeps its last requirement.
    Clear(ResolutionTarget),
}

/// `sources supersedes`.
#[derive(Subcommand)]
pub enum SourceListCommand {
    /// Add one source to the list.
    Add(SourceTarget),
    /// Remove one source from the list.
    Clear(SourceTarget),
}

/// `questions contradicts`.
#[derive(Subcommand)]
pub enum QuestionSingleCommand {
    /// Name the requirement this question's requirement contradicts.
    Set(QuestionTarget),
    /// Empty the field.
    Clear(QuestionOnly),
}
