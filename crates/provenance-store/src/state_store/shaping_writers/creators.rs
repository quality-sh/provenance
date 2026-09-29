use super::{artifact_links::sort_artifact_links, ensure_within_read_budget};
use crate::{
    shards,
    state_store::{CreateBoundaryInput, CreateQuestionInput, CreateTopicInput, StateStore},
};
use provenance_core::{Boundary, NodeType, Question, Topic, SUPPORTED_SCHEMA_VERSION};

impl StateStore {
    pub fn create_boundary(&self, input: CreateBoundaryInput) -> anyhow::Result<Boundary> {
        let path = shards::boundaries_path(&self.layout, &input.scope_id);
        let id = input.id.clone();
        self.create_native_record(&path, &id, |store| store.write_boundary(input))
    }

    fn write_boundary(&self, input: CreateBoundaryInput) -> anyhow::Result<Boundary> {
        let CreateBoundaryInput {
            scope_id,
            id,
            requirement_id,
            statement,
            source_ref,
        } = input;
        self.ensure_canonical_id_available(&scope_id, &id)?;
        crate::write_error::ensure!(
            InvalidUpdate,
            self.list_requirements(&scope_id)?
                .iter()
                .any(|requirement| requirement.id == requirement_id),
            "requirement does not exist"
        );
        if let Some(source_ref) = &source_ref {
            crate::write_error::ensure!(
                InvalidUpdate,
                self.list_sources(&scope_id)?
                    .iter()
                    .any(|source| source.id == source_ref.source_id),
                "source does not exist"
            );
        }
        let path = shards::boundaries_path(&self.layout, &scope_id);
        self.mutate_graph_record(&path, |records: &mut Vec<Boundary>| {
            let boundary = Boundary {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: scope_id.clone(),
                id,
                requirement_id,
                statement,
                source_ref,
            };
            crate::write_error::ensure!(
                InvalidUpdate,
                !records.iter().any(|record| record.id == boundary.id),
                "boundary already exists"
            );
            ensure_within_read_budget(&boundary)?;
            records.push(boundary.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(boundary)
        })
    }

    pub fn create_topic(&self, input: CreateTopicInput) -> anyhow::Result<Topic> {
        let path = shards::topics_path(&self.layout, &input.scope_id);
        let id = input.id.clone();
        self.create_native_record(&path, &id, |store| store.write_topic(input))
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
        self.ensure_canonical_id_available(&scope_id, &id)?;
        crate::write_error::ensure!(
            InvalidUpdate,
            self.list_requirements(&scope_id)?
                .iter()
                .any(|requirement| requirement.id == requirement_id),
            "requirement does not exist"
        );
        self.validate_artifact_links(&scope_id, &links)?;
        sort_artifact_links(&mut links);
        let path = shards::topics_path(&self.layout, &scope_id);
        self.mutate_graph_record(&path, |records: &mut Vec<Topic>| {
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
            crate::write_error::ensure!(
                InvalidUpdate,
                !records.iter().any(|record| record.id == topic.id),
                "topic already exists"
            );
            ensure_within_read_budget(&topic)?;
            records.push(topic.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(topic)
        })
    }

    pub fn create_question(&self, input: CreateQuestionInput) -> anyhow::Result<Question> {
        let path = shards::questions_path(&self.layout, &input.scope_id);
        let id = input.id.clone();
        self.create_native_record(&path, &id, |store| store.write_question(input))
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
            contradicts,
        } = input;
        self.ensure_canonical_id_available(&scope_id, &id)?;
        let topic = self
            .list_topics(&scope_id)?
            .into_iter()
            .find(|topic| topic.id == topic_id)
            .ok_or_else(|| {
                crate::write_error::SourceFailure::wrap(
                    crate::write_error::WriteFailure::MissingReference,
                    anyhow::anyhow!("topic does not exist"),
                )
            })?;
        if let Some(resolution_id) = &resolution_id {
            self.ensure_node_exists(
                &scope_id,
                NodeType::Resolution,
                resolution_id,
                "--resolution-id",
            )?;
        }
        if let Some(contradicted) = &contradicts {
            self.ensure_node_exists(
                &scope_id,
                NodeType::Requirement,
                contradicted,
                "--contradicts",
            )?;
        }
        self.validate_artifact_links(&scope_id, &links)?;
        sort_artifact_links(&mut links);
        let path = shards::questions_path(&self.layout, &scope_id);
        self.mutate_graph_record(&path, |records: &mut Vec<Question>| {
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
                contradicts,
            };
            crate::write_error::ensure!(
                InvalidUpdate,
                !records.iter().any(|record| record.id == question.id),
                "question already exists"
            );
            ensure_within_read_budget(&question)?;
            records.push(question.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(question)
        })
    }
}
