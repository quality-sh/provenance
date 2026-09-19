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
type ResponseMeta = { stamp?: Stamp | null; freshness_error?: string | null; freshness_cause?: "catch_up_failed" | null; limit?: number | null; has_more?: boolean | null; next_cursor?: string | null };

export interface GetRequest<Kind extends NodeType = NodeType> { node_type: Kind; id: string }
export interface SearchRequest { collection: GraphCollection; text: string; limit?: number; cursor?: string }
export interface NeighborsRequest { node_type: NodeType; id: string; direction?: Direction; limit?: number }
export interface TraceRequest { node_type: NodeType; id: string; direction?: Direction; max_depth?: number }
export interface ImpactRequest { node_type: NodeType; id: string }
export interface ResolveSymbolRequest { file: string; symbol?: string; line?: number }
export interface EvidenceRequest { rule: string; base?: string; head?: string }
export interface StaleRequest { base?: string; head?: string; limit?: number; cursor?: string }
export type GraphCollection = "sources" | "requirements" | "resolutions" | "rules" | "domains" | "boundaries" | "topics" | "questions";

type MemberResponse<Response> = Response extends { data: infer Data }
  ? Data extends { id: unknown; schema_version: number; scope_id: unknown } ? Response : never
  : never;
type ResourceGetResponses = {
  source: MemberResponse<Schemas["GetSourceSuccess"]>;
  requirement: MemberResponse<Schemas["GetRequirementSuccess"]>;
  resolution: MemberResponse<Schemas["GetResolutionSuccess"]>;
  rule: MemberResponse<Schemas["GetRuleSuccess"]>;
  domain: MemberResponse<Schemas["GetDomainSuccess"]>;
  boundary: MemberResponse<Schemas["GetBoundarySuccess"]>;
  topic: MemberResponse<Schemas["GetTopicSuccess"]>;
  question: MemberResponse<Schemas["GetQuestionSuccess"]>;
};
export type GetResponse<Kind extends NodeType = NodeType> = ResourceGetResponses[Kind];
export type SearchResponse = { data: { items: GraphNode[] }; meta: ResponseMeta };
export type Neighbor = Schemas["GetRuleSuccessNeighbor"];
export type NeighborsResponse = { data: { id: string; neighbors: Neighbor[] }; meta: ResponseMeta };
export type TracedNode = Schemas["GetRuleSuccessTracedNode"];
export type TraceResponse = { data: { id: string; max_depth: number; nodes: TracedNode[] }; meta: ResponseMeta };
export type AffectedRule = Schemas["GetRuleSuccessAffectedRule"];
export type ImpactResponse = { data: { id: string; affected_rules: AffectedRule[]; scan_cut: boolean }; meta: ResponseMeta };
export type ResolveSymbolResponse = SearchResponse;
export type EvidenceResponse = Schemas["GetRuleEvidenceSuccess"];
export type EvidenceDiffSite = Schemas["ListRulesSuccessEvidenceDiffSite"];
export type EvidenceDiffState = EvidenceDiffSite["state"];
export type EvidenceSiteKind = EvidenceDiffSite["kind"];
export type EvidenceDiffSummary = Schemas["ListRulesSuccessEvidenceDiffSummary"];
export type StaleResponse = { data: { items: EvidenceDiffSite[] }; meta: ResponseMeta };
export type QueryEnvelope = { meta: ResponseMeta };
export type ImplementationSite = AffectedRule["implementations"][number];
export type VerificationSite = AffectedRule["verifications"][number];
export type ImplementationBinding = Schemas["GetRuleEvidenceSuccessImplementationBinding"];
export type VerificationBinding = Schemas["ListVerificationBindingsSuccessVerificationBinding"];
export type VerificationRun = Schemas["ListVerificationRunsSuccessVerificationRun"];
export type RequirementReview = Schemas["GetRuleEvidenceSuccessRequirementReview"];
export type StaleEvidence = Schemas["GetRuleEvidenceSuccessStaleEvidence"];
