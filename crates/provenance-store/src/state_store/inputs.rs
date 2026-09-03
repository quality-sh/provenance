use provenance_core::{
    ArtifactLink, CanonicalArtifact, ClaimChallenge, ConsensusFinding, ContestedClaim,
    ContributionStance, DeclarationAddress, DispositionActor, DispositionDecision, EdgeType,
    EvidenceGap, IdeationEvidenceReference, IdeationTarget, MaterialClaim, MessageRole,
    MinorityObjection, NodeType, PromotionState, ProposalTraceability, ProposalType,
    QuestionStatus, RequiredHumanDecision, RequirementStatus, ResolutionInput, ResolutionMethod,
    ResolutionStatus, RuleSeverity, RuleStatus, ScopeId, SourceReference, SourceType, StableId,
    SuggestedArtifact, SuggestedArtifactChange, ThreadParent, TopicStatus, UncertaintyRating,
    UnsupportedRecommendation, UnsupportedSpeculation, VerificationMethod,
};
use serde::{Deserialize, Serialize};

pub struct CreateSourceInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub name: String,
    pub source_type: SourceType,
    pub url: Option<String>,
    pub reference: Option<String>,
    pub commit_pin: Option<String>,
    pub effective_date: Option<i64>,
    pub review_date: Option<i64>,
    pub supersedes: Vec<StableId>,
    pub origin_thread: Option<StableId>,
    pub origin_message: Option<StableId>,
}

pub struct CreateRequirementInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub statement: String,
    pub description: Option<String>,
    pub status: RequirementStatus,
    pub domain_id: Option<StableId>,
    pub refines: Option<StableId>,
    pub depends_on: Vec<StableId>,
    pub supersedes: Vec<StableId>,
    pub spawned_by: Option<StableId>,
    pub origin_thread: Option<StableId>,
    pub origin_message: Option<StableId>,
}

pub struct CreateDomainInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
}

pub struct AddSourceReferenceInput {
    pub scope_id: ScopeId,
    pub source_id: StableId,
    pub requirement_id: StableId,
    pub clause: Option<String>,
}

pub struct CreateEdgeInput {
    pub scope_id: ScopeId,
    pub edge_type: EdgeType,
    pub from_type: NodeType,
    pub from_id: StableId,
    pub to_type: NodeType,
    pub to_id: StableId,
}

pub struct CreateBoundaryInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub requirement_id: StableId,
    pub statement: String,
    pub source_ref: Option<SourceReference>,
}

pub struct CreateTopicInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub requirement_id: StableId,
    pub title: String,
    pub status: TopicStatus,
    pub links: Vec<ArtifactLink>,
}

pub struct CreateQuestionInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub topic_id: StableId,
    pub question: String,
    pub resolution_method: ResolutionMethod,
    pub status: QuestionStatus,
    pub answer: Option<String>,
    pub links: Vec<ArtifactLink>,
    pub resolution_id: Option<StableId>,
    pub contradicts: Option<StableId>,
}

pub struct UpdateQuestionInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub resolution_method: Option<ResolutionMethod>,
    pub status: Option<QuestionStatus>,
    pub links: Option<Vec<ArtifactLink>>,
    pub resolution_id: Option<StableId>,
}

pub struct CreateResolutionInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub title: String,
    pub requirement_ids: Vec<StableId>,
    pub supersedes: Vec<StableId>,
    pub position: String,
    pub rationale: String,
    pub status: ResolutionStatus,
    pub context: Option<String>,
    pub enforcement: Option<String>,
    pub confidence: Option<f64>,
    pub inputs: Vec<ResolutionInput>,
    pub made_by: Option<String>,
    pub approved_by: Option<String>,
    pub approved_at: Option<i64>,
    pub origin_thread: Option<StableId>,
    pub origin_message: Option<StableId>,
}

pub struct CreateRuleInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub name: Option<String>,
    pub description: Option<String>,
    pub requirement_ids: Vec<StableId>,
    pub resolution_ids: Vec<StableId>,
    pub statement: String,
    pub status: RuleStatus,
    pub severity: RuleSeverity,
    pub source_document: Option<String>,
    pub source_section: Option<String>,
    pub origin_thread: Option<StableId>,
    pub origin_message: Option<StableId>,
}

// The typed-spec input family is wire protocol and lives in core; these
// re-exports keep the store paths every existing consumer uses.
pub use provenance_core::protocol::{
    TypedAdoptionTarget, TypedDeclarationKind, TypedImplementationInput, TypedRequirementInput,
    TypedRuleInput, TypedSourceInput, TypedSpecInput,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TypedResourceKind {
    Source,
    Requirement,
    Rule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconcileState {
    Created,
    Updated,
    Moved,
    Retired,
    Conflict,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ReconciledResource {
    pub kind: TypedResourceKind,
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub address: provenance_core::DeclarationAddress,
    pub id: StableId,
    pub state: ReconcileState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<TypedFieldChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TypedFieldChange {
    pub field: String,
    pub before: serde_json::Value,
    pub after: serde_json::Value,
}

/// One ASD-STE100 violation attached to its typed declaration site.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TypedSpecDiagnostic {
    pub address: DeclarationAddress,
    pub resource_kind: TypedResourceKind,
    pub field: String,
    pub standard: provenance_ste100::Standard,
    pub issue: provenance_ste100::StandardIssue,
    pub rule: provenance_ste100::RuleNumber,
    pub disposition: provenance_ste100::FindingKind,
    pub span: provenance_ste100::Span,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TypedSpecResult {
    pub declared_by: String,
    pub created: usize,
    pub updated: usize,
    pub moved: usize,
    pub retired: usize,
    pub conflicts: usize,
    pub unchanged: usize,
    pub resources: Vec<ReconciledResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<TypedSpecDiagnostic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub implementation_bindings: Vec<provenance_core::ImplementationBinding>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginVerificationInput {
    #[serde(default)]
    pub rule: Option<String>,
    #[serde(default)]
    pub declaration: Option<DeclarationReferenceInput>,
    pub key: String,
    pub method: String,
    pub declared_by: String,
    #[serde(default)]
    pub file: Option<camino::Utf8PathBuf>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub commit: Option<String>,
}

pub struct MaterializeVerificationBindingInput {
    pub scope_id: ScopeId,
    pub rule_id: StableId,
    pub key: String,
    pub method: VerificationMethod,
    pub declared_by: String,
    pub file: camino::Utf8PathBuf,
    pub symbol: Option<String>,
}

pub struct MaterializeImplementationBindingInput {
    pub scope_id: ScopeId,
    pub rule_id: StableId,
    pub declared_by: String,
    pub file: camino::Utf8PathBuf,
    pub symbol: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclarationReferenceInput {
    pub declared_by: String,
    pub address: DeclarationAddress,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompleteVerificationInput {
    pub run: String,
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
}

pub struct PostMessageInput {
    pub scope_id: ScopeId,
    pub parent: ThreadParent,
    pub role: MessageRole,
    pub body: String,
}

pub struct CreateContributionInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub target: IdeationTarget,
    pub participant_slot: String,
    pub stance: ContributionStance,
    pub strongest_finding: String,
    pub evidence_references: Vec<IdeationEvidenceReference>,
    pub material_claims: Vec<MaterialClaim>,
    pub risks: Vec<String>,
    pub objections: Vec<String>,
    pub challenges: Vec<ClaimChallenge>,
    pub suggested_artifact_changes: Vec<SuggestedArtifactChange>,
    pub unsupported_recommendations: Vec<UnsupportedRecommendation>,
    pub uncertainty: UncertaintyRating,
    pub open_questions: Vec<String>,
}

pub struct CreateSynthesisPacketInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub target: IdeationTarget,
    pub summary: String,
    pub consensus: Vec<ConsensusFinding>,
    pub contested_claims: Vec<ContestedClaim>,
    pub minority_objections: Vec<MinorityObjection>,
    pub evidence_gaps: Vec<EvidenceGap>,
    pub unsupported_speculation: Vec<UnsupportedSpeculation>,
    pub open_questions: Vec<String>,
    pub suggested_artifacts: Vec<SuggestedArtifact>,
    pub required_human_decisions: Vec<RequiredHumanDecision>,
}

pub struct CreateProposalCardInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub proposal_key: String,
    pub proposal_type: ProposalType,
    pub title: String,
    pub summary: String,
    pub confidence: Option<f64>,
    pub traceability: ProposalTraceability,
    pub builds_on: Vec<provenance_core::AssertionId>,
    pub promotion_state: PromotionState,
    pub duplicate_of: Option<StableId>,
    pub superseded_by: Option<StableId>,
}

pub struct CreateDispositionInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub proposal_id: StableId,
    pub decision: DispositionDecision,
    pub rationale: String,
    pub actor: DispositionActor,
    pub canonical_artifact: Option<CanonicalArtifact>,
    pub external_action: Option<provenance_core::ExternalActionCorrelation>,
}

pub struct CreateAssertionInput {
    pub scope_id: ScopeId,
    pub id: provenance_core::AssertionId,
    pub proposal_id: StableId,
    pub synthesis_packet_id: StableId,
    pub supporting_claim_ids: Vec<StableId>,
}
