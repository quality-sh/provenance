use super::{CreateBoundaryInput, CreateQuestionInput, CreateTopicInput, StateStore};
use crate::shards;
use provenance_core::{Boundary, Question, Topic, SUPPORTED_SCHEMA_VERSION};

mod artifact_links;

#[allow(clippy::redundant_pub_crate)]
pub(in crate::state_store) use artifact_links::sort_artifact_links;

impl StateStore {
    pub fn create_boundary(&self, input: CreateBoundaryInput) -> anyhow::Result<Boundary> {
        self.with_repository_publication(|| self.write_boundary(input))
    }

    fn write_boundary(&self, input: CreateBoundaryInput) -> anyhow::Result<Boundary> {
        let CreateBoundaryInput {
            scope_id,
            id,
            requirement_id,
            statement,
            source_ref,
        } = input;
        anyhow::ensure!(
            self.list_requirements(&scope_id)?
                .iter()
                .any(|requirement| requirement.id == requirement_id),
            "requirement does not exist"
        );
        if let Some(source_ref) = &source_ref {
            anyhow::ensure!(
                self.list_sources(&scope_id)?
                    .iter()
                    .any(|source| source.id == source_ref.source_id),
                "source does not exist"
            );
        }
        let path = shards::boundaries_path(&self.layout, &scope_id);
        self.mutate_jsonl_records(&path, |records: &mut Vec<Boundary>| {
            let boundary = Boundary {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: scope_id.clone(),
                id,
                requirement_id,
                statement,
                source_ref,
            };
            anyhow::ensure!(
                !records.iter().any(|record| record.id == boundary.id),
                "boundary already exists"
            );
            records.push(boundary.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(boundary)
        })
    }

    pub fn create_topic(&self, input: CreateTopicInput) -> anyhow::Result<Topic> {
        self.with_repository_publication(|| self.write_topic(input))
    }

    fn write_topic(&self, input: CreateTopicInput) -> anyhow::Result<Topic> {
        let CreateTopicInput {
            scope_id,
            id,
            requirement_id,
            title,
            status,
            mut links,
        } = input;
        anyhow::ensure!(
            self.list_requirements(&scope_id)?
                .iter()
                .any(|requirement| requirement.id == requirement_id),
            "requirement does not exist"
        );
        self.validate_artifact_links(&scope_id, &links)?;
        sort_artifact_links(&mut links);
        let path = shards::topics_path(&self.layout, &scope_id);
        self.mutate_jsonl_records(&path, |records: &mut Vec<Topic>| {
            let topic = Topic {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: scope_id.clone(),
                id,
                requirement_id,
                title,
                status,
                claimed_by: None,
                claimed_at: None,
                links,
            };
            anyhow::ensure!(
                !records.iter().any(|record| record.id == topic.id),
                "topic already exists"
            );
            records.push(topic.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(topic)
        })
    }

    pub fn create_question(&self, input: CreateQuestionInput) -> anyhow::Result<Question> {
        self.with_repository_publication(|| self.write_question(input))
    }

    fn write_question(&self, input: CreateQuestionInput) -> anyhow::Result<Question> {
        let CreateQuestionInput {
            scope_id,
            id,
            topic_id,
            question,
            resolution_method,
            status,
            answer,
            mut links,
            resolution_id,
        } = input;
        let topic = self
            .list_topics(&scope_id)?
            .into_iter()
            .find(|topic| topic.id == topic_id)
            .ok_or_else(|| anyhow::anyhow!("topic does not exist"))?;
        if let Some(resolution_id) = &resolution_id {
            anyhow::ensure!(
                self.list_resolutions(&scope_id)?
                    .iter()
                    .any(|resolution| &resolution.id == resolution_id),
                "resolution does not exist"
            );
        }
        self.validate_artifact_links(&scope_id, &links)?;
        sort_artifact_links(&mut links);
        let path = shards::questions_path(&self.layout, &scope_id);
        self.mutate_jsonl_records(&path, |records: &mut Vec<Question>| {
            let question = Question {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: scope_id.clone(),
                id,
                topic_id,
                requirement_id: topic.requirement_id,
                question,
                resolution_method,
                status,
                claimed_by: None,
                claimed_at: None,
                answer,
                links,
                resolution_id,
            };
            anyhow::ensure!(
                !records.iter().any(|record| record.id == question.id),
                "question already exists"
            );
            records.push(question.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(question)
        })
    }
}
