use super::common::{parse_json_arg, warn_if_skills_missing};
use super::references;
use crate::cli::shaping::QuestionsCommand;
use crate::output;
use provenance_core::{ArtifactLink, QuestionStatus, ResolutionMethod, ScopeId, StableId};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{CreateQuestionInput, StateStore, UpdateQuestionInput},
};

#[allow(clippy::too_many_lines)]
pub(super) fn handle(command: QuestionsCommand, quiet: bool) -> anyhow::Result<()> {
    match command {
        QuestionsCommand::Create {
            repo,
            scope,
            id,
            topic_id,
            question,
            method,
            status,
            answer,
            links_json,
            resolution_id,
            contradicts,
            format,
        } => {
            warn_if_skills_missing(&repo, quiet)?;
            let question = StateStore::new(ProvenanceLayout::new(repo)).create_question(
                CreateQuestionInput {
                    scope_id: ScopeId::new(scope)?,
                    id: StableId::new(id)?,
                    topic_id: StableId::new(topic_id)?,
                    question,
                    resolution_method: ResolutionMethod::parse(&method)?,
                    status: QuestionStatus::parse(&status)?,
                    answer,
                    links: parse_json_arg::<Vec<ArtifactLink>>("links-json", &links_json)?,
                    resolution_id: resolution_id.map(StableId::new).transpose()?,
                    contradicts: contradicts.map(StableId::new).transpose()?,
                },
            )?;
            output::print(format, &question)?;
        }
        QuestionsCommand::Contradicts { command } => references::question_contradicts(command)?,
        QuestionsCommand::List {
            repo,
            scope,
            format,
        } => {
            warn_if_skills_missing(&repo, quiet)?;
            let questions = StateStore::new(ProvenanceLayout::new(repo))
                .list_questions(&ScopeId::new(scope)?)?;
            output::print(format, &questions)?;
        }
        QuestionsCommand::Update {
            repo,
            scope,
            id,
            method,
            status,
            links_json,
            resolution_id,
            format,
        } => {
            warn_if_skills_missing(&repo, quiet)?;
            let question = StateStore::new(ProvenanceLayout::new(repo)).update_question(
                UpdateQuestionInput {
                    scope_id: ScopeId::new(scope)?,
                    id: StableId::new(id)?,
                    resolution_method: method
                        .map(|value| ResolutionMethod::parse(&value))
                        .transpose()?,
                    status: status
                        .map(|value| QuestionStatus::parse(&value))
                        .transpose()?,
                    links: links_json
                        .map(|value| parse_json_arg::<Vec<ArtifactLink>>("links-json", &value))
                        .transpose()?,
                    resolution_id: resolution_id.map(StableId::new).transpose()?,
                },
            )?;
            output::print(format, &question)?;
        }
        QuestionsCommand::Claim {
            repo,
            scope,
            id,
            actor,
            format,
        } => {
            warn_if_skills_missing(&repo, quiet)?;
            let question = StateStore::new(ProvenanceLayout::new(repo)).claim_question(
                &ScopeId::new(scope)?,
                &StableId::new(id)?,
                &actor,
            )?;
            output::print(format, &question)?;
        }
        QuestionsCommand::Release {
            repo,
            scope,
            id,
            format,
        } => {
            warn_if_skills_missing(&repo, quiet)?;
            let question = StateStore::new(ProvenanceLayout::new(repo))
                .release_question(&ScopeId::new(scope)?, &StableId::new(id)?)?;
            output::print(format, &question)?;
        }
        QuestionsCommand::Answer {
            repo,
            scope,
            id,
            answer,
            resolution_id,
            format,
        } => {
            warn_if_skills_missing(&repo, quiet)?;
            let question = StateStore::new(ProvenanceLayout::new(repo)).answer_question(
                &ScopeId::new(scope)?,
                &StableId::new(id)?,
                answer,
                resolution_id.map(StableId::new).transpose()?,
            )?;
            output::print(format, &question)?;
        }
    }
    Ok(())
}
