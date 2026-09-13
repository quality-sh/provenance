use super::common::parse_json_arg;
use crate::cli::shaping::TopicsCommand;
use crate::output;
use crate::store::Store;
use provenance_core::{ArtifactLink, ScopeId, StableId, TopicStatus};
use provenance_store::state_store::CreateTopicInput;

pub(super) async fn handle(command: TopicsCommand) -> anyhow::Result<()> {
    match command {
        TopicsCommand::Update(args) => {
            super::updates::handle::<provenance_store::operations::catalog::UpdateTopic>(args)
                .await?;
        }
        TopicsCommand::Create {
            repo,
            scope,
            id,
            requirement_id,
            title,
            status,
            links_json,
            ..
        } => {
            let topic = Store::open(repo).create_topic(CreateTopicInput {
                scope_id: ScopeId::new(scope)?,
                id: StableId::new(id)?,
                requirement_id: StableId::new(requirement_id)?,
                title,
                status: TopicStatus::parse(&status)?,
                links: parse_json_arg::<Vec<ArtifactLink>>("links-json", &links_json)?,
            })?;
            output::print_json(&topic)?;
        }
        TopicsCommand::List { repo, scope, .. } => {
            let topics = Store::open(repo).list_topics(&ScopeId::new(scope)?)?;
            output::print_json(&topics)?;
        }
        TopicsCommand::Claim {
            repo,
            scope,
            id,
            actor,
            ..
        } => {
            let store = Store::open(repo);
            let scope = ScopeId::new(scope)?;
            let topic_id = StableId::new(id)?;
            let claim = store.claim_topic(&scope, &topic_id, &actor)?;
            output::print_json(&claim)?;
        }
        TopicsCommand::Release {
            repo, scope, id, ..
        } => {
            let topic =
                Store::open(repo).release_topic(&ScopeId::new(scope)?, &StableId::new(id)?)?;
            output::print_json(&topic)?;
        }
        TopicsCommand::Close {
            repo, scope, id, ..
        } => {
            let topic =
                Store::open(repo).close_topic(&ScopeId::new(scope)?, &StableId::new(id)?)?;
            output::print_json(&topic)?;
        }
    }
    Ok(())
}
