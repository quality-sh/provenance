//! Reference add, set, and clear commands run catalog operations.
//!
//! One table in this module maps every command to the catalog operation it
//! runs. The args type carries the owner flag, and the executor carries the
//! field kind: a targeted command names the owner and the target, an
//! owner-only command names the owner alone. Scope context, id parsing, and
//! output are shared, so a new command is one table entry.

use super::native::invoke_native;
use crate::cli::references::{
    QuestionOnly, QuestionSingleCommand, QuestionTarget, RequirementListCommand, RequirementOnly,
    RequirementSingleCommand, RequirementTarget, ResolutionListCommand, ResolutionTarget,
    RuleListCommand, RuleTarget, SourceListCommand, SourceTarget,
};
use crate::output::{self, OutputFormat};
use camino::Utf8PathBuf;
use provenance_core::{ScopeId, StableId};
use provenance_store::operations::catalog;

/// Which field a requirement single-target command addresses.
#[derive(Clone, Copy)]
pub(super) enum RequirementSingle {
    Refines,
    SpawnedBy,
}

/// Which field a requirement list command addresses.
#[derive(Clone, Copy)]
pub(super) enum RequirementList {
    DependsOn,
    Supersedes,
}

/// Which field a rule list command addresses.
#[derive(Clone, Copy)]
pub(super) enum RuleList {
    Requirement,
    Resolution,
}

/// Which field a resolution list command addresses.
#[derive(Clone, Copy)]
pub(super) enum ResolutionList {
    Requirement,
    Supersedes,
}

/// The flags every reference command carries.
trait RefCommand {
    fn repo(&self) -> &Utf8PathBuf;
    fn scope(&self) -> &str;
    /// The flag that names the record which owns the field.
    fn owner(&self) -> &str;
    fn format(&self) -> OutputFormat;
}

/// A command that also names the record the field points at.
trait WithTarget: RefCommand {
    fn target(&self) -> &str;
}

impl RefCommand for RequirementTarget {
    fn repo(&self) -> &Utf8PathBuf {
        &self.repo
    }
    fn scope(&self) -> &str {
        &self.scope
    }
    fn owner(&self) -> &str {
        &self.requirement_id
    }
    fn format(&self) -> OutputFormat {
        self.format
    }
}
impl WithTarget for RequirementTarget {
    fn target(&self) -> &str {
        &self.target_id
    }
}

impl RefCommand for RequirementOnly {
    fn repo(&self) -> &Utf8PathBuf {
        &self.repo
    }
    fn scope(&self) -> &str {
        &self.scope
    }
    fn owner(&self) -> &str {
        &self.requirement_id
    }
    fn format(&self) -> OutputFormat {
        self.format
    }
}

impl RefCommand for RuleTarget {
    fn repo(&self) -> &Utf8PathBuf {
        &self.repo
    }
    fn scope(&self) -> &str {
        &self.scope
    }
    fn owner(&self) -> &str {
        &self.rule_id
    }
    fn format(&self) -> OutputFormat {
        self.format
    }
}
impl WithTarget for RuleTarget {
    fn target(&self) -> &str {
        &self.target_id
    }
}

impl RefCommand for ResolutionTarget {
    fn repo(&self) -> &Utf8PathBuf {
        &self.repo
    }
    fn scope(&self) -> &str {
        &self.scope
    }
    fn owner(&self) -> &str {
        &self.resolution_id
    }
    fn format(&self) -> OutputFormat {
        self.format
    }
}
impl WithTarget for ResolutionTarget {
    fn target(&self) -> &str {
        &self.target_id
    }
}

impl RefCommand for SourceTarget {
    fn repo(&self) -> &Utf8PathBuf {
        &self.repo
    }
    fn scope(&self) -> &str {
        &self.scope
    }
    fn owner(&self) -> &str {
        &self.source_id
    }
    fn format(&self) -> OutputFormat {
        self.format
    }
}
impl WithTarget for SourceTarget {
    fn target(&self) -> &str {
        &self.target_id
    }
}

impl RefCommand for QuestionTarget {
    fn repo(&self) -> &Utf8PathBuf {
        &self.repo
    }
    fn scope(&self) -> &str {
        &self.scope
    }
    fn owner(&self) -> &str {
        &self.id
    }
    fn format(&self) -> OutputFormat {
        self.format
    }
}
impl WithTarget for QuestionTarget {
    fn target(&self) -> &str {
        &self.target_id
    }
}

impl RefCommand for QuestionOnly {
    fn repo(&self) -> &Utf8PathBuf {
        &self.repo
    }
    fn scope(&self) -> &str {
        &self.scope
    }
    fn owner(&self) -> &str {
        &self.id
    }
    fn format(&self) -> OutputFormat {
        self.format
    }
}

/// Runs an operation whose request names the owner and the target, and
/// prints the record it returns.
async fn targeted<O>(args: &impl WithTarget) -> anyhow::Result<()>
where
    O: catalog::Operation<Request = catalog::ReferenceActionInput>,
{
    let scope = ScopeId::new(args.scope())?;
    let request = catalog::ReferenceActionInput {
        scope_id: scope.clone(),
        id: StableId::new(args.owner())?,
        target_id: StableId::new(args.target())?,
    };
    let record = invoke_native::<O>(args.repo().clone(), scope, request).await?;
    output::print(args.format(), &record)
}

/// Runs an operation whose request names only the owner, and prints the
/// record it returns.
async fn owner_only<O>(args: &impl RefCommand) -> anyhow::Result<()>
where
    O: catalog::Operation<Request = catalog::RecordActionInput>,
{
    let scope = ScopeId::new(args.scope())?;
    let request = catalog::RecordActionInput {
        scope_id: scope.clone(),
        id: StableId::new(args.owner())?,
    };
    let record = invoke_native::<O>(args.repo().clone(), scope, request).await?;
    output::print(args.format(), &record)
}

/// `requirements refines` and `requirements spawned-by`: one target at most.
pub(super) async fn requirement_single(
    field: RequirementSingle,
    command: RequirementSingleCommand,
) -> anyhow::Result<()> {
    match (field, command) {
        (RequirementSingle::Refines, RequirementSingleCommand::Set(args)) => {
            targeted::<catalog::SetRequirementRefines>(&args).await
        }
        (RequirementSingle::Refines, RequirementSingleCommand::Clear(args)) => {
            owner_only::<catalog::ClearRequirementRefines>(&args).await
        }
        (RequirementSingle::SpawnedBy, RequirementSingleCommand::Set(args)) => {
            targeted::<catalog::SetRequirementSpawnedBy>(&args).await
        }
        (RequirementSingle::SpawnedBy, RequirementSingleCommand::Clear(args)) => {
            owner_only::<catalog::ClearRequirementSpawnedBy>(&args).await
        }
    }
}

/// `requirements depends-on` and `requirements supersedes`: a list.
pub(super) async fn requirement_list(
    field: RequirementList,
    command: RequirementListCommand,
) -> anyhow::Result<()> {
    match (field, command) {
        (RequirementList::DependsOn, RequirementListCommand::Add(args)) => {
            targeted::<catalog::AddRequirementDependsOn>(&args).await
        }
        (RequirementList::DependsOn, RequirementListCommand::Clear(args)) => {
            targeted::<catalog::ClearRequirementDependsOn>(&args).await
        }
        (RequirementList::Supersedes, RequirementListCommand::Add(args)) => {
            targeted::<catalog::AddRequirementSupersedes>(&args).await
        }
        (RequirementList::Supersedes, RequirementListCommand::Clear(args)) => {
            targeted::<catalog::ClearRequirementSupersedes>(&args).await
        }
    }
}

/// `rules requirement` and `rules resolution`.
pub(super) async fn rule_list(
    field: RuleList,
    command: RuleListCommand,
) -> anyhow::Result<()> {
    match (field, command) {
        (RuleList::Requirement, RuleListCommand::Add(args)) => {
            targeted::<catalog::AddRuleRequirement>(&args).await
        }
        (RuleList::Requirement, RuleListCommand::Clear(args)) => {
            targeted::<catalog::ClearRuleRequirement>(&args).await
        }
        (RuleList::Resolution, RuleListCommand::Add(args)) => {
            targeted::<catalog::AddRuleResolution>(&args).await
        }
        (RuleList::Resolution, RuleListCommand::Clear(args)) => {
            targeted::<catalog::ClearRuleResolution>(&args).await
        }
    }
}

/// `resolutions requirement` and `resolutions supersedes`.
pub(super) async fn resolution_list(
    field: ResolutionList,
    command: ResolutionListCommand,
) -> anyhow::Result<()> {
    match (field, command) {
        (ResolutionList::Requirement, ResolutionListCommand::Add(args)) => {
            targeted::<catalog::AddResolutionRequirement>(&args).await
        }
        (ResolutionList::Requirement, ResolutionListCommand::Clear(args)) => {
            targeted::<catalog::ClearResolutionRequirement>(&args).await
        }
        (ResolutionList::Supersedes, ResolutionListCommand::Add(args)) => {
            targeted::<catalog::AddResolutionSupersedes>(&args).await
        }
        (ResolutionList::Supersedes, ResolutionListCommand::Clear(args)) => {
            targeted::<catalog::ClearResolutionSupersedes>(&args).await
        }
    }
}

/// `sources supersedes`.
pub(super) async fn source_supersedes(command: SourceListCommand) -> anyhow::Result<()> {
    match command {
        SourceListCommand::Add(args) => targeted::<catalog::AddSourceSupersedes>(&args).await,
        SourceListCommand::Clear(args) => targeted::<catalog::ClearSourceSupersedes>(&args).await,
    }
}

/// `questions contradicts`.
pub(super) async fn question_contradicts(command: QuestionSingleCommand) -> anyhow::Result<()> {
    match command {
        QuestionSingleCommand::Set(args) => targeted::<catalog::SetQuestionContradicts>(&args).await,
        QuestionSingleCommand::Clear(args) => {
            owner_only::<catalog::ClearQuestionContradicts>(&args).await
        }
    }
}
