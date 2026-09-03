//! Dispatch for the reference add, set, and clear commands.

use crate::cli::references::{
    QuestionSingleCommand, RequirementListCommand, RequirementSingleCommand, ResolutionListCommand,
    RuleListCommand, SourceListCommand,
};
use crate::output;
use provenance_core::{ScopeId, StableId};
use provenance_store::{layout::ProvenanceLayout, state_store::StateStore};

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

#[derive(Clone, Copy)]
pub(super) enum RuleList {
    Requirement,
    Resolution,
}

#[derive(Clone, Copy)]
pub(super) enum ResolutionList {
    Requirement,
    Supersedes,
}

pub(super) fn requirement_single(
    field: RequirementSingle,
    command: RequirementSingleCommand,
) -> anyhow::Result<()> {
    match command {
        RequirementSingleCommand::Set(args) => {
            let store = StateStore::new(ProvenanceLayout::new(args.repo));
            let scope = ScopeId::new(args.scope)?;
            let requirement = StableId::new(args.requirement_id)?;
            let target = StableId::new(args.target_id)?;
            let record = match field {
                RequirementSingle::Refines => {
                    store.set_requirement_refines(&scope, &requirement, target)?
                }
                RequirementSingle::SpawnedBy => {
                    store.set_requirement_spawned_by(&scope, &requirement, target)?
                }
            };
            output::print(args.format, &record)
        }
        RequirementSingleCommand::Clear(args) => {
            let store = StateStore::new(ProvenanceLayout::new(args.repo));
            let scope = ScopeId::new(args.scope)?;
            let requirement = StableId::new(args.requirement_id)?;
            let record = match field {
                RequirementSingle::Refines => {
                    store.clear_requirement_refines(&scope, &requirement)?
                }
                RequirementSingle::SpawnedBy => {
                    store.clear_requirement_spawned_by(&scope, &requirement)?
                }
            };
            output::print(args.format, &record)
        }
    }
}

pub(super) fn requirement_list(
    field: RequirementList,
    command: RequirementListCommand,
) -> anyhow::Result<()> {
    let (args, add) = match command {
        RequirementListCommand::Add(args) => (args, true),
        RequirementListCommand::Clear(args) => (args, false),
    };
    let store = StateStore::new(ProvenanceLayout::new(args.repo));
    let scope = ScopeId::new(args.scope)?;
    let requirement = StableId::new(args.requirement_id)?;
    let target = StableId::new(args.target_id)?;
    let record = match (field, add) {
        (RequirementList::DependsOn, true) => {
            store.add_requirement_depends_on(&scope, &requirement, target)?
        }
        (RequirementList::DependsOn, false) => {
            store.clear_requirement_depends_on(&scope, &requirement, &target)?
        }
        (RequirementList::Supersedes, true) => {
            store.add_requirement_supersedes(&scope, &requirement, target)?
        }
        (RequirementList::Supersedes, false) => {
            store.clear_requirement_supersedes(&scope, &requirement, &target)?
        }
    };
    output::print(args.format, &record)
}

pub(super) fn rule_list(field: RuleList, command: RuleListCommand) -> anyhow::Result<()> {
    let (args, add) = match command {
        RuleListCommand::Add(args) => (args, true),
        RuleListCommand::Clear(args) => (args, false),
    };
    let store = StateStore::new(ProvenanceLayout::new(args.repo));
    let scope = ScopeId::new(args.scope)?;
    let rule = StableId::new(args.rule_id)?;
    let target = StableId::new(args.target_id)?;
    let record = match (field, add) {
        (RuleList::Requirement, true) => store.add_rule_requirement(&scope, &rule, target)?,
        (RuleList::Requirement, false) => store.clear_rule_requirement(&scope, &rule, &target)?,
        (RuleList::Resolution, true) => store.add_rule_resolution(&scope, &rule, target)?,
        (RuleList::Resolution, false) => store.clear_rule_resolution(&scope, &rule, &target)?,
    };
    output::print(args.format, &record)
}

pub(super) fn resolution_list(
    field: ResolutionList,
    command: ResolutionListCommand,
) -> anyhow::Result<()> {
    let (args, add) = match command {
        ResolutionListCommand::Add(args) => (args, true),
        ResolutionListCommand::Clear(args) => (args, false),
    };
    let store = StateStore::new(ProvenanceLayout::new(args.repo));
    let scope = ScopeId::new(args.scope)?;
    let resolution = StableId::new(args.resolution_id)?;
    let target = StableId::new(args.target_id)?;
    let record = match (field, add) {
        (ResolutionList::Requirement, true) => {
            store.add_resolution_requirement(&scope, &resolution, target)?
        }
        (ResolutionList::Requirement, false) => {
            store.clear_resolution_requirement(&scope, &resolution, &target)?
        }
        (ResolutionList::Supersedes, true) => {
            store.add_resolution_supersedes(&scope, &resolution, target)?
        }
        (ResolutionList::Supersedes, false) => {
            store.clear_resolution_supersedes(&scope, &resolution, &target)?
        }
    };
    output::print(args.format, &record)
}

pub(super) fn source_supersedes(command: SourceListCommand) -> anyhow::Result<()> {
    let (args, add) = match command {
        SourceListCommand::Add(args) => (args, true),
        SourceListCommand::Clear(args) => (args, false),
    };
    let store = StateStore::new(ProvenanceLayout::new(args.repo));
    let scope = ScopeId::new(args.scope)?;
    let source = StableId::new(args.source_id)?;
    let target = StableId::new(args.target_id)?;
    let record = if add {
        store.add_source_supersedes(&scope, &source, target)?
    } else {
        store.clear_source_supersedes(&scope, &source, &target)?
    };
    output::print(args.format, &record)
}

pub(super) fn question_contradicts(command: QuestionSingleCommand) -> anyhow::Result<()> {
    match command {
        QuestionSingleCommand::Set(args) => {
            let store = StateStore::new(ProvenanceLayout::new(args.repo));
            let record = store.set_question_contradicts(
                &ScopeId::new(args.scope)?,
                &StableId::new(args.id)?,
                StableId::new(args.target_id)?,
            )?;
            output::print(args.format, &record)
        }
        QuestionSingleCommand::Clear(args) => {
            let store = StateStore::new(ProvenanceLayout::new(args.repo));
            let record = store
                .clear_question_contradicts(&ScopeId::new(args.scope)?, &StableId::new(args.id)?)?;
            output::print(args.format, &record)
        }
    }
}
