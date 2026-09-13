mod evidence;
use provenance_core::{
    Manifest, RepoPathPrefix, RequirementStatus, RuleSeverity, RuleStatus, ScopeId, StableId,
};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{
        CreateBoundaryInput, CreateDomainInput, CreateQuestionInput, CreateRequirementInput,
        CreateResolutionInput, CreateRuleInput, CreateSourceInput, CreateTopicInput, StateStore,
    },
};
use serde_json::{json, Value};
use std::fmt::Write;

pub struct Repository {
    pub dir: tempfile::TempDir,
    pub layout: ProvenanceLayout,
}
impl Repository {
    pub fn new(statement: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let layout = ProvenanceLayout::new(dir.path().to_str().unwrap());
        std::fs::create_dir_all(layout.state_dir()).unwrap();
        let scope = ScopeId::new("default").unwrap();
        let manifest = Manifest::default_with_scope(scope.clone(), RepoPathPrefix::new("."));
        std::fs::write(
            layout.manifest_path(),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let result = Self { dir, layout };
        result.seed(scope, statement);
        result
    }
    pub fn seed(&self, scope: ScopeId, statement: &str) {
        let store = StateStore::new(self.layout.clone());
        store
            .create_domain(CreateDomainInput {
                scope_id: scope.clone(),
                id: sid("domain_shared"),
                name: "Shared domain".into(),
                description: None,
                color: None,
            })
            .unwrap();
        store
            .create_requirement(CreateRequirementInput {
                scope_id: scope.clone(),
                id: sid("req_shared"),
                statement: "The graph is readable.".into(),
                description: None,
                status: RequirementStatus::Active,
                domain_id: Some(sid("domain_shared")),
                refines: None,
                depends_on: vec![],
                supersedes: vec![],
                spawned_by: None,
                origin_thread: None,
                origin_message: None,
            })
            .unwrap();
        store
            .create_rule(CreateRuleInput {
                archived_in_commit: None,
                scope_id: scope,
                id: sid("rule_shared"),
                name: None,
                description: None,
                requirement_ids: vec![sid("req_shared")],
                resolution_ids: vec![],
                statement: statement.into(),
                status: RuleStatus::Active,
                severity: RuleSeverity::High,
                source_document: None,
                source_section: None,
                origin_thread: None,
                origin_message: None,
            })
            .unwrap();
    }
    pub fn add_scope(&self, name: &str, statement: &str) {
        let mut manifest: Manifest =
            serde_json::from_slice(&std::fs::read(self.layout.manifest_path()).unwrap()).unwrap();
        let scope = ScopeId::new(name).unwrap();
        manifest.scopes.push(provenance_core::Scope {
            id: scope.clone(),
            path_prefix: RepoPathPrefix::new("."),
        });
        std::fs::write(
            self.layout.manifest_path(),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        self.seed(scope, statement);
    }
    pub fn all_kinds(&self) {
        use provenance_core::{
            QuestionStatus, ResolutionMethod, ResolutionStatus, SourceType, TopicStatus,
        };
        let scope = ScopeId::new("default").unwrap();
        let store = StateStore::new(self.layout.clone());
        store
            .create_source(CreateSourceInput {
                scope_id: scope.clone(),
                id: sid("source_shared"),
                name: "Shared source".into(),
                source_type: SourceType::Document,
                url: None,
                reference: None,
                commit_pin: None,
                effective_date: None,
                review_date: None,
                supersedes: vec![],
                origin_thread: None,
                origin_message: None,
            })
            .unwrap();
        store
            .create_boundary(CreateBoundaryInput {
                scope_id: scope.clone(),
                id: sid("boundary_shared"),
                requirement_id: sid("req_shared"),
                statement: "The shared graph excludes code.".into(),
                source_ref: None,
            })
            .unwrap();
        store
            .create_topic(CreateTopicInput {
                scope_id: scope.clone(),
                id: sid("topic_shared"),
                requirement_id: sid("req_shared"),
                title: "Shared topic".into(),
                status: TopicStatus::Open,
                links: vec![],
            })
            .unwrap();
        store
            .create_question(CreateQuestionInput {
                scope_id: scope.clone(),
                id: sid("question_shared"),
                topic_id: sid("topic_shared"),
                question: "Which shared record applies?".into(),
                resolution_method: ResolutionMethod::Research,
                status: QuestionStatus::Open,
                answer: None,
                links: vec![],
                resolution_id: None,
                contradicts: None,
            })
            .unwrap();
        store
            .create_resolution(CreateResolutionInput {
                scope_id: scope,
                id: sid("resolution_shared"),
                title: "Shared resolution".into(),
                requirement_ids: vec![sid("req_shared")],
                supersedes: vec![],
                position: "The shared graph is readable.".into(),
                rationale: "The reader uses the shared contract.".into(),
                status: ResolutionStatus::Draft,
                context: None,
                enforcement: None,
                confidence: None,
                inputs: vec![],
                made_by: None,
                approved_by: None,
                approved_at: None,
                origin_thread: None,
                origin_message: None,
            })
            .unwrap();
    }
    pub fn bytes(&self) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
        fn visit(
            root: &std::path::Path,
            path: &std::path::Path,
            found: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
        ) {
            for entry in std::fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_dir() {
                    found.insert(path.strip_prefix(root).unwrap().to_path_buf(), vec![]);
                    visit(root, &path, found);
                } else {
                    found.insert(
                        path.strip_prefix(root).unwrap().to_path_buf(),
                        std::fs::read(&path).unwrap(),
                    );
                }
            }
        }
        let mut found = std::collections::BTreeMap::new();
        visit(self.dir.path(), self.dir.path(), &mut found);
        found
    }
    pub fn edit(&self, scope: &str, text: &str) {
        let path =
            provenance_store::shards::rules_path(&self.layout, &ScopeId::new(scope).unwrap());
        let mut records: Vec<Value> = std::fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        records[0]["statement"] = json!(text);
        let mut encoded = String::new();
        for record in records {
            writeln!(encoded, "{record}").unwrap();
        }
        std::fs::write(path, encoded).unwrap();
    }
}
fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}
