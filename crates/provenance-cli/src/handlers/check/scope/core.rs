use crate::handlers::check::index::CheckIndex;
use crate::handlers::check::references::{
    check_artifact_links, check_origin_references, check_scoped_reference,
};
use provenance_core::model::relations::{declaration_of, kind_word, RelationOwner};
use provenance_core::{
    Boundary, Domain, ImplementationBinding, Question, Requirement, Resolution, Rule, ScopeId,
    Source, Topic, VerificationBinding,
};
use provenance_store::state_store::StateStore;

pub(super) struct Records {
    sources: Vec<Source>,
    domains: Vec<Domain>,
    requirements: Vec<Requirement>,
    boundaries: Vec<Boundary>,
    topics: Vec<Topic>,
    questions: Vec<Question>,
    resolutions: Vec<Resolution>,
    rules: Vec<Rule>,
    verification_bindings: Vec<VerificationBinding>,
    implementation_bindings: Vec<ImplementationBinding>,
}

impl Records {
    pub(super) fn load(store: &StateStore, scope_id: &ScopeId) -> anyhow::Result<Self> {
        Ok(Self {
            sources: store.list_sources(scope_id)?,
            domains: store.list_domains(scope_id)?,
            requirements: store.list_requirements(scope_id)?,
            boundaries: store.list_boundaries(scope_id)?,
            topics: store.list_topics(scope_id)?,
            questions: store.list_questions(scope_id)?,
            resolutions: store.list_resolutions(scope_id)?,
            rules: store.list_rules(scope_id)?,
            verification_bindings: store.list_verification_bindings(scope_id)?,
            implementation_bindings: store.list_implementation_bindings(scope_id)?,
        })
    }

    pub(super) fn validate_scope_ownership(
        &self,
        loaded_scope_id: &ScopeId,
        findings: &mut Vec<String>,
    ) {
        macro_rules! check_records {
            ($records:expr, $record_type:literal) => {
                for record in $records {
                    super::check_scope_ownership(
                        loaded_scope_id,
                        &record.scope_id,
                        $record_type,
                        &record.id,
                        findings,
                    );
                }
            };
        }

        check_records!(&self.sources, "source");
        check_records!(&self.domains, "domain");
        check_records!(&self.requirements, "requirement");
        check_records!(&self.boundaries, "boundary");
        check_records!(&self.topics, "topic");
        check_records!(&self.questions, "question");
        check_records!(&self.resolutions, "resolution");
        check_records!(&self.rules, "rule");
        check_records!(&self.verification_bindings, "verification binding");
        check_records!(&self.implementation_bindings, "implementation binding");
    }

    pub(super) fn add_to(&self, index: &mut CheckIndex) {
        for source in &self.sources {
            index.add_node(&source.scope_id, "source", &source.id);
        }
        for domain in &self.domains {
            index.add_node(&domain.scope_id, "domain", &domain.id);
        }
        for requirement in &self.requirements {
            index.add_node(&requirement.scope_id, "requirement", &requirement.id);
        }
        for boundary in &self.boundaries {
            index.add_node(&boundary.scope_id, "boundary", &boundary.id);
        }
        for topic in &self.topics {
            index.add_node(&topic.scope_id, "topic", &topic.id);
        }
        for question in &self.questions {
            index.add_node(&question.scope_id, "question", &question.id);
        }
        for resolution in &self.resolutions {
            index.add_node(&resolution.scope_id, "resolution", &resolution.id);
        }
        for rule in &self.rules {
            index.add_node(&rule.scope_id, "rule", &rule.id);
        }
    }

    pub(super) fn validate(
        &self,
        index: &CheckIndex,
        scope_id: &ScopeId,
        dangling: &mut Vec<String>,
    ) {
        validate_declared_relations(self, index, scope_id, dangling);
        validate_links_and_origins(self, index, scope_id, dangling);
        validate_verification_bindings(self, index, scope_id, dangling);
        validate_implementation_bindings(self, index, scope_id, dangling);
    }
}

fn validate_implementation_bindings(
    records: &Records,
    index: &CheckIndex,
    scope_id: &ScopeId,
    dangling: &mut Vec<String>,
) {
    let mut ids = std::collections::BTreeSet::new();
    let mut rules = std::collections::BTreeSet::new();
    for binding in &records.implementation_bindings {
        let owner = format!("implementation binding {}", binding.id.as_str());
        if !ids.insert(binding.id.as_str()) {
            dangling.push(format!(
                "duplicate implementation binding id {}",
                binding.id.as_str()
            ));
        }
        if !rules.insert(binding.rule_id.as_str()) {
            dangling.push(format!(
                "more than one canonical primary implementation binding for rule {}",
                binding.rule_id.as_str()
            ));
        }
        if binding.declared_by.trim().is_empty() {
            dangling.push(format!("{owner} declared_by must not be empty"));
        }
        if binding.file.as_str().is_empty() {
            dangling.push(format!("{owner} file must not be empty"));
        } else if !is_portable_repository_path(&binding.file) {
            dangling.push(format!("{owner} file must be repository-relative"));
        }
        if binding.symbol.trim().is_empty() {
            dangling.push(format!("{owner} symbol must not be empty"));
        }
        check_scoped_reference(
            index,
            dangling,
            scope_id,
            &owner,
            "rule_id",
            "rule",
            &binding.rule_id,
        );
    }
}

fn validate_verification_bindings(
    records: &Records,
    index: &CheckIndex,
    scope_id: &ScopeId,
    dangling: &mut Vec<String>,
) {
    let mut ids = std::collections::BTreeSet::new();
    for binding in &records.verification_bindings {
        let owner = format!("verification binding {}", binding.id.as_str());
        if !ids.insert(binding.id.as_str()) {
            dangling.push(format!(
                "duplicate verification binding id {}",
                binding.id.as_str()
            ));
        }
        if binding.file.as_str().is_empty() {
            dangling.push(format!("{owner} file must not be empty"));
        } else if !is_portable_repository_path(&binding.file) {
            dangling.push(format!("{owner} file must be repository-relative"));
        }
        check_scoped_reference(
            index,
            dangling,
            scope_id,
            &owner,
            "rule_id",
            "rule",
            &binding.rule_id,
        );
    }
}

fn is_portable_repository_path(path: &camino::Utf8Path) -> bool {
    !path.as_str().contains('\\')
        && !path.is_absolute()
        && !path.components().any(|component| {
            matches!(
                component,
                camino::Utf8Component::ParentDir
                    | camino::Utf8Component::RootDir
                    | camino::Utf8Component::Prefix(_)
            )
        })
}

/// One pass over every declared relation of every owner kind: each
/// reference must name a record of the declared kind in this scope.
fn validate_declared_relations(
    records: &Records,
    index: &CheckIndex,
    scope_id: &ScopeId,
    dangling: &mut Vec<String>,
) {
    check_declared(index, dangling, scope_id, &records.sources);
    check_declared(index, dangling, scope_id, &records.requirements);
    check_declared(index, dangling, scope_id, &records.resolutions);
    check_declared(index, dangling, scope_id, &records.rules);
    check_declared(index, dangling, scope_id, &records.topics);
    check_declared(index, dangling, scope_id, &records.questions);
    check_declared(index, dangling, scope_id, &records.boundaries);
}

fn check_declared<T: RelationOwner>(
    index: &CheckIndex,
    dangling: &mut Vec<String>,
    scope_id: &ScopeId,
    records: &[T],
) {
    for record in records {
        let owner = format!("{} {}", kind_word(T::OWNER), record.id().as_str());
        for (name, target) in record.references() {
            let decl =
                declaration_of(T::relations(), name).expect("references name declared fields");
            check_scoped_reference(
                index,
                dangling,
                scope_id,
                &owner,
                name,
                kind_word(decl.target),
                target,
            );
        }
    }
}

/// The links topics and questions carry, and the thread origins on the
/// four authored kinds: neither is a declared relation.
fn validate_links_and_origins(
    records: &Records,
    index: &CheckIndex,
    scope_id: &ScopeId,
    dangling: &mut Vec<String>,
) {
    for source in &records.sources {
        check_origin_references(
            index,
            dangling,
            scope_id,
            &format!("source {}", source.id.as_str()),
            source.origin_thread.as_ref(),
            source.origin_message.as_ref(),
        );
    }
    for requirement in &records.requirements {
        check_origin_references(
            index,
            dangling,
            scope_id,
            &format!("requirement {}", requirement.id.as_str()),
            requirement.origin_thread.as_ref(),
            requirement.origin_message.as_ref(),
        );
    }
    for topic in &records.topics {
        let owner = format!("topic {}", topic.id.as_str());
        check_artifact_links(index, dangling, scope_id, &owner, &topic.links);
    }
    for question in &records.questions {
        let owner = format!("question {}", question.id.as_str());
        check_artifact_links(index, dangling, scope_id, &owner, &question.links);
    }
    for resolution in &records.resolutions {
        check_origin_references(
            index,
            dangling,
            scope_id,
            &format!("resolution {}", resolution.id.as_str()),
            resolution.origin_thread.as_ref(),
            resolution.origin_message.as_ref(),
        );
    }
    for rule in &records.rules {
        check_origin_references(
            index,
            dangling,
            scope_id,
            &format!("rule {}", rule.id.as_str()),
            rule.origin_thread.as_ref(),
            rule.origin_message.as_ref(),
        );
    }
}
