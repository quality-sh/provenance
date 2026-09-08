import type { components } from "./generated/schema.js";

type Schemas = components["schemas"];

// Fluent builders use omitted optional values and readonly addresses. Wire
// requests retain the generated nullable fields and mutable JSON arrays.
type NonNullFields<T> = { [Key in keyof T]: NonNullable<T[Key]> };
type WireSpecDocument = Schemas["ApplyRequestInput"]["request"];
type WireRule = NonNullable<WireSpecDocument["rules"]>[number];
export type DeclarationAddress = readonly string[];
export type AdoptionTarget = NonNullable<WireSpecDocument["adopt_unowned"]>[number];
export type ResourceKind = AdoptionTarget["kind"];
export type SourceKind = Schemas["GetSuccessOutputSourceType"];
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

export type ApplyResult = Schemas["ApplySuccessOutput"];
export type PlanResult = Schemas["PlanSuccessOutput"];
export type ReconciledResource = ApplyResult["resources"][number];
export type ReconcileState = ReconciledResource["state"];
export type FieldChange = NonNullable<ReconciledResource["changes"]>[number];
export type TypedSpecDiagnostic = NonNullable<ApplyResult["diagnostics"]>[number];
export type RuleEvidence = PlanResult["affected_rules"][number]["evidence"];
export type ReviewReason = NonNullable<RuleEvidence["reasons"]>[number];

export type GetRequest = Schemas["GetRequestInput"]["request"];
export type GetResponse = Schemas["GetSuccessOutput"];
export type SearchRequest = Schemas["SearchRequestInput"]["request"];
export type SearchResponse = Schemas["SearchSuccessOutput"];
export type NeighborsRequest = Schemas["NeighborsRequestInput"]["request"];
export type NeighborsResponse = Schemas["NeighborsSuccessOutput"];
export type TraceRequest = Schemas["TraceRequestInput"]["request"];
export type TraceResponse = Schemas["TraceSuccessOutput"];
export type ImpactRequest = Schemas["ImpactRequestInput"]["request"];
export type ImpactResponse = Schemas["ImpactSuccessOutput"];
export type ResolveSymbolRequest = Schemas["ResolveSymbolRequestInput"]["request"];
export type ResolveSymbolResponse = Schemas["ResolveSymbolSuccessOutput"];
export type EvidenceRequest = Schemas["EvidenceRequestInput"]["request"];
export type EvidenceResponse = Schemas["EvidenceSuccessOutput"];
export type StaleRequest = Schemas["StaleRequestInput"]["request"];
export type StaleResponse = Schemas["StaleSuccessOutput"];

export type NodeType = GetRequest["node_type"];
export type Direction = NonNullable<NeighborsRequest["direction"]>;
export type GraphNode = NonNullable<GetResponse["node"]>;
export type Stamp = GetResponse["stamp"];
export type StampPolicy = Stamp["policy"];
export type LiveWord = Stamp["live"][number];

type QueryResponse = GetResponse | SearchResponse | NeighborsResponse | TraceResponse
  | ImpactResponse | ResolveSymbolResponse | EvidenceResponse | StaleResponse;
/** The common fields of current generated query responses. */
export type QueryEnvelope = Pick<QueryResponse,
  "protocol_version" | "operation" | "stamp" | "freshness_error" | "freshness_cause">;

export type Neighbor = NeighborsResponse["neighbors"][number];
export type TracedNode = TraceResponse["nodes"][number];
export type AffectedRule = ImpactResponse["affected_rules"][number];
export type ImplementationSite = AffectedRule["implementations"][number];
export type VerificationSite = AffectedRule["verifications"][number];
export type ImplementationBinding = EvidenceResponse["implementation_bindings"][number];
export type VerificationBinding = Schemas["VerificationBindingsSuccessOutput"][number];
export type VerificationRun = Schemas["VerificationRunsSuccessOutput"][number];
export type RequirementReview = EvidenceResponse["reviews"][number];
export type EvidenceDiffSite = StaleResponse["sites"][number];
export type EvidenceDiffState = EvidenceDiffSite["state"];
export type EvidenceSiteKind = EvidenceDiffSite["kind"];
export type EvidenceDiffSummary = StaleResponse["summary"];
export type StaleEvidence = NonNullable<EvidenceResponse["stale"]>;
