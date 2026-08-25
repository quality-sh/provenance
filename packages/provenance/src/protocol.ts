export type DeclarationAddress = readonly string[];
export type ResourceKind = "source" | "requirement" | "rule";

export interface AdoptionTarget {
  kind: ResourceKind;
  id: string;
}

export interface SourceDeclaration {
  key: string;
  id?: string;
  name: string;
  kind: string;
  url?: string;
  reference?: string;
}

export interface RequirementDeclaration {
  key: string;
  id?: string;
  statement: string;
  description?: string;
  sources: string[];
}

export interface RuleDeclaration {
  key: string;
  id?: string;
  address?: DeclarationAddress;
  requirement?: string;
  requirements?: string[];
  statement: string;
  name?: string;
  description?: string;
  implementation?: ImplementationDeclaration;
}

export interface ImplementationDeclaration {
  file: string;
  symbol: string;
}

export interface TypedSpecDocument {
  schema_version: 1;
  spec: string;
  declared_by: string;
  adopt_unowned?: AdoptionTarget[];
  sources: SourceDeclaration[];
  requirements: RequirementDeclaration[];
  rules: RuleDeclaration[];
}

export type ReconcileState =
  | "created"
  | "updated"
  | "moved"
  | "retired"
  | "conflict"
  | "unchanged";

export interface ReconciledResource {
  kind: ResourceKind;
  key: string;
  parent?: string;
  address: string[];
  id: string;
  state: ReconcileState;
  changes?: FieldChange[];
}

export interface FieldChange {
  field: string;
  before: unknown;
  after: unknown;
}

export interface ApplyResult {
  declared_by: string;
  created: number;
  updated: number;
  moved: number;
  retired: number;
  conflicts: number;
  unchanged: number;
  resources: ReconciledResource[];
  diagnostics?: TypedSpecDiagnostic[];
  implementation_bindings?: ImplementationBinding[];
}

export interface TypedSpecDiagnostic {
  address: string[];
  resource_kind: "requirement" | "rule";
  field: "statement";
  standard: "ASD-STE100";
  issue: 9;
  rule: "8.1";
  disposition: "violation";
  span: { start: number; end: number };
  message: string;
}

export interface ImplementationBinding {
  id: string;
  rule_id: string;
  declared_by: string;
  retired?: boolean;
  file: string;
  symbol: string;
}

export interface ImplementationSite {
  file: string;
  line?: number;
  symbol?: string;
}

export interface VerificationSite {
  key?: string;
  method: string;
  declared_by?: string;
  file: string;
  line?: number;
  symbol?: string;
}

export interface ReviewReason {
  requirement: string;
  field: string;
  before: string;
  after: string;
  changed_at?: number;
}

export interface RuleEvidence {
  review_required: boolean;
  reasons?: ReviewReason[];
}

export interface AffectedRule {
  id: string;
  implementations: ImplementationSite[];
  verifications: VerificationSite[];
  evidence?: RuleEvidence;
}

export interface PlanResult extends ApplyResult {
  affected_rules: AffectedRule[];
}

export interface VerificationRun {
  id: string;
  binding_id: string;
  rule_id: string;
  commit?: string;
  file?: string;
  symbol?: string;
  status: "running" | "passed" | "failed";
}

export type NodeType =
  | "source"
  | "requirement"
  | "resolution"
  | "rule"
  | "topic"
  | "question";

export type EdgeType =
  | "references"
  | "refines_into"
  | "depends_on"
  | "contradicts"
  | "supersedes"
  | "needs"
  | "resolves"
  | "spawns"
  | "produces";

export type Direction = "out" | "in" | "both";

/** One canonical record as the engine hands it back. */
export interface GraphNode {
  node_type: NodeType;
  id: string;
  retired?: boolean;
  [field: string]: unknown;
}

/** Every structured query answers under this envelope. */
export interface QueryEnvelope {
  protocol_version: number;
  operation: string;
}

interface QueryRequest {
  protocol_version?: number;
  include_retired?: boolean;
}

interface PagedRequest extends QueryRequest {
  limit?: number;
}

interface PagedResponse extends QueryEnvelope {
  limit: number;
  has_more: boolean;
}

export interface GetRequest extends QueryRequest {
  node_type: NodeType;
  id: string;
}

export interface GetResponse extends QueryEnvelope {
  found: boolean;
  node?: GraphNode;
}

export interface SearchRequest extends PagedRequest {
  text: string;
  node_types?: NodeType[];
}

export interface SearchResponse extends PagedResponse {
  nodes: GraphNode[];
}

export interface Neighbor {
  edge_type: EdgeType;
  direction: "out" | "in";
  node: GraphNode;
}

export interface NeighborsRequest extends PagedRequest {
  id: string;
  node_type?: NodeType;
  direction?: Direction;
  edge_types?: EdgeType[];
}

export interface NeighborsResponse extends PagedResponse {
  id: string;
  neighbors: Neighbor[];
}

export interface TracedNode {
  depth: number;
  node: GraphNode;
}

export interface TraceRequest extends NeighborsRequest {
  max_depth?: number;
}

export interface TraceResponse extends PagedResponse {
  id: string;
  max_depth: number;
  nodes: TracedNode[];
}

export interface ImpactRequest extends PagedRequest {
  id: string;
  node_type?: NodeType;
}

export interface ImpactResponse extends PagedResponse {
  id: string;
  affected_rules: AffectedRule[];
}

export interface VerificationBinding {
  id: string;
  rule_id: string;
  key: string;
  method: string;
  declared_by: string;
  retired?: boolean;
  file: string;
  symbol?: string;
}

export interface RequirementReview {
  id: string;
  rule_id: string;
  requirement_id: string;
  field: string;
  before: string;
  after: string;
  changed_at: number;
  cleared_at?: number;
  cleared_by_run?: string;
}

export type EvidenceDiffState = "untouched" | "touched" | "moved" | "gone";

export type EvidenceSiteKind =
  | "rule_binding"
  | "verification"
  | "annotation"
  | "source_reference";

export interface EvidenceDiffSite {
  kind: EvidenceSiteKind;
  subject_id: string;
  file_path: string;
  line?: number;
  end_line?: number;
  state: EvidenceDiffState;
  original_file_path?: string;
  original_line?: number;
}

export interface EvidenceDiffSummary {
  total_sites: number;
  untouched: number;
  touched: number;
  moved: number;
  gone: number;
}

/** What a commit range did to the code carrying a Rule's evidence. */
export interface StaleEvidence {
  base: string;
  head: string;
  sites: EvidenceDiffSite[];
}

export interface EvidenceRequest extends PagedRequest {
  rule: string;
  base?: string;
  head?: string;
}

export interface EvidenceResponse extends PagedResponse {
  rule_id: string;
  implementation_bindings: ImplementationBinding[];
  verification_bindings: VerificationBinding[];
  verification_runs: VerificationRun[];
  latest_verification_run?: VerificationRun;
  review_required: boolean;
  reviews: RequirementReview[];
  stale: StaleEvidence | null;
}

export interface StaleRequest extends PagedRequest {
  base: string;
  head?: string;
  rules?: string[];
}

export interface StaleResponse extends PagedResponse {
  base: string;
  head: string;
  files_changed: number;
  summary: EvidenceDiffSummary;
  sites: EvidenceDiffSite[];
}

export interface ResolveSymbolRequest extends PagedRequest {
  file: string;
  symbol?: string;
  line?: number;
}

export interface ResolveSymbolResponse extends PagedResponse {
  file: string;
  symbol?: string;
  rules: GraphNode[];
}
