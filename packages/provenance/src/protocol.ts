import type { components } from "./generated/schema.js";

type Schemas = components["schemas"];

// Fluent builders use omitted optional values and readonly addresses. Wire
// requests retain the generated nullable fields and mutable JSON arrays.
type NonNullFields<T> = { [Key in keyof T]: NonNullable<T[Key]> };
type WireSpecDocument = Schemas["ApplyAuthoringRequest"]["data"];
type WireRule = NonNullable<WireSpecDocument["rules"]>[number];
export type DeclarationAddress = readonly string[];
export type AdoptionTarget = NonNullable<WireSpecDocument["adopt_unowned"]>[number];
export type ResourceKind = AdoptionTarget["kind"];
export type SourceKind = Schemas["GetSourceSuccessSourceType"];
export type SourceDeclaration = NonNullFields<NonNullable<WireSpecDocument["sources"]>[number]>;
export type RequirementDeclaration = NonNullFields<NonNullable<WireSpecDocument["requirements"]>[number]> & {
  sources: string[];
};
export type RuleDeclaration = Omit<NonNullFields<WireRule>, "address"> & {
  address?: DeclarationAddress;
};
export type ImplementationDeclaration = NonNullable<RuleDeclaration["implementation"]>;

/** The state schema version emitted by the local authoring builders. */
export const STATE_SCHEMA_VERSION = 2;
export type TypedSpecDocument = Omit<NonNullFields<WireSpecDocument>, "schema_version" | "sources" | "requirements" | "rules"> & {
  schema_version: typeof STATE_SCHEMA_VERSION;
  sources: SourceDeclaration[];
  requirements: RequirementDeclaration[];
  rules: RuleDeclaration[];
};

export type ApplyResult = Schemas["ApplyAuthoringSuccess"]["data"];
export type PlanResult = Schemas["PlanAuthoringSuccess"]["data"];
export type ReconciledResource = ApplyResult["resources"][number];
export type ReconcileState = ReconciledResource["state"];
export type FieldChange = NonNullable<ReconciledResource["changes"]>[number];
export type TypedSpecDiagnostic = NonNullable<ApplyResult["diagnostics"]>[number];
export type RuleEvidence = PlanResult["affected_rules"][number]["evidence"];
export type ReviewReason = NonNullable<RuleEvidence["reasons"]>[number];

export type NodeType = Schemas["ListRulesSuccessGraphNode"]["node_type"];
export type Direction = Schemas["GetRuleSuccessDirection"];
export type GraphNode = Schemas["ListRulesSuccessGraphNode"];
export type Stamp = Schemas["GetRuleSuccessResponseMetaStamp"];
export type StampPolicy = Stamp["policy"];
export type LiveWord = Stamp["live"][number];

export interface GetRequest<Kind extends NodeType = NodeType> { node_type: Kind; id: string }
export interface SearchRequest { collection: GraphCollection; text: string; limit?: number; cursor?: string }
export interface NeighborsRequest { node_type: NodeType; id: string; direction?: Direction; limit?: number }
export interface TraceRequest { node_type: NodeType; id: string; direction?: Direction; max_depth?: number }
export interface ImpactRequest { node_type: NodeType; id: string }
export interface ResolveSymbolRequest { file: string; symbol?: string; line?: number }
export interface EvidenceRequest { rule: string; base?: string; head?: string }
export interface StaleRequest { base: string; head?: string; limit?: number; cursor?: string }
export type GraphCollection = "sources" | "requirements" | "resolutions" | "rules" | "domains" | "boundaries" | "topics" | "questions";

type MemberResponse<Response> = Response extends { data: infer Data }
  ? Data extends { id: unknown; schema_version: number; scope_id: unknown } ? Response : never
  : never;
type ResourceGetResponses = {
  source: MemberResponse<Schemas["GetSourceBaseSuccess"]>;
  requirement: MemberResponse<Schemas["GetRequirementBaseSuccess"]>;
  resolution: MemberResponse<Schemas["GetResolutionBaseSuccess"]>;
  rule: MemberResponse<Schemas["GetRuleBaseSuccess"]>;
  domain: MemberResponse<Schemas["GetDomainBaseSuccess"]>;
  boundary: MemberResponse<Schemas["GetBoundaryBaseSuccess"]>;
  topic: MemberResponse<Schemas["GetTopicBaseSuccess"]>;
  question: MemberResponse<Schemas["GetQuestionBaseSuccess"]>;
};
export type GetResponse<Kind extends NodeType = NodeType> = ResourceGetResponses[Kind];
export type SearchResponse = Schemas["ListRulesSearchSuccess"];
export type Neighbor = Schemas["GetRuleSuccessNeighbor"];
export type NeighborsResponse = Schemas["GetRuleNeighborsSuccess"];
export type TracedNode = Schemas["GetRuleSuccessTracedNode"];
export type TraceResponse = Schemas["GetRuleTraceSuccess"];
export type AffectedRule = Schemas["GetRuleSuccessAffectedRule"];
export type ImpactResponse = Schemas["GetRuleImpactSuccess"];
export type ResolveSymbolResponse = Schemas["ListRulesResolveSymbolSuccess"];
export type EvidenceResponse = Schemas["GetRuleEvidenceSuccess"];
export type EvidenceDiffSite = Schemas["ListRulesSuccessEvidenceDiffSite"];
export type EvidenceDiffState = EvidenceDiffSite["state"];
export type EvidenceSiteKind = EvidenceDiffSite["kind"];
export type EvidenceDiffSummary = Schemas["ListRulesSuccessEvidenceDiffSummary"];
export type StaleResponse = Schemas["ListRulesStaleSuccess"];
export type QueryEnvelope = { meta: SearchResponse["meta"] };
export type ImplementationSite = AffectedRule["implementations"][number];
export type VerificationSite = AffectedRule["verifications"][number];
export type ImplementationBinding = Schemas["GetRuleEvidenceSuccessImplementationBinding"];
export type VerificationBinding = Schemas["ListVerificationBindingsSuccessVerificationBinding"];
export type VerificationRun = Schemas["ListVerificationRunsSuccessVerificationRun"];
export type RequirementReview = Schemas["GetRuleEvidenceSuccessRequirementReview"];
export type StaleEvidence = Schemas["GetRuleEvidenceSuccessStaleEvidence"];
