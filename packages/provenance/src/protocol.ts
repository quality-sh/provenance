export type DeclarationAddress = readonly string[];
export type ResourceKind = "source" | "requirement" | "rule";

export interface AdoptionTarget {
  kind: ResourceKind;
  id: string;
}

// The canonical source types that fluent authoring selects. It mirrors
// the Rust `SourceType` enum. The wire protocol also maps the provider
// aliases `linear`, `github`, and `jira` onto `external_integration`;
// fluent authoring states the canonical value instead.
export type SourceKind =
  | "policy"
  | "document"
  | "legislation"
  | "company_agreement"
  | "system_state"
  | "external_integration"
  | "domain_knowledge"
  | "project_artifact"
  | "incident"
  | "api_spec";

export interface SourceDeclaration {
  key: string;
  id?: string;
  name: string;
  kind: string;
  url?: string;
  reference?: string;
  /** Keys of older sources in the same document this one replaces. */
  supersedes?: string[];
}

export interface RequirementDeclaration {
  key: string;
  id?: string;
  statement: string;
  description?: string;
  sources: string[];
  /** The key of the requirement in the same document this one refines. */
  refines?: string;
  /** Keys of requirements in the same document this one depends on. */
  depends_on?: string[];
  /** Keys of older requirements in the same document this one replaces. */
  supersedes?: string[];
  /** The canonical id of the resolution this requirement came out of. */
  spawned_by?: string;
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
  /** Canonical ids of the resolutions this rule follows from. */
  resolution_ids?: string[];
}

export interface ImplementationDeclaration {
  file: string;
  symbol: string;
}

/** The state schema version every typed spec document names. */
export const STATE_SCHEMA_VERSION = 2;

export interface TypedSpecDocument {
  schema_version: typeof STATE_SCHEMA_VERSION;
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
  | "question"
  | "domain"
  | "boundary";

export type Direction = "out" | "in" | "both";

/** One canonical record as the engine hands it back. */
export interface GraphNode {
  node_type: NodeType;
  id: string;
  retired?: boolean;
  [field: string]: unknown;
}

/** The freshness step a read ran before it answered. */
export type StampPolicy =
  | "catch_up"
  | "annotate_only"
  | "refuse_stale"
  | "catch_up_failed";

/**
 * What a stamp does not cover, from a closed list: canonical shards, a
 * working-tree scan, the verification run file, or a commit range.
 */
export type LiveWord = "canonical" | "scanned_sites" | "verification_runs" | "diff";

/**
 * What a query answer reflects. `serial` and `digest` name the projection
 * revision the rows came from and `instance_id` the projection instance;
 * serials compare only within one instance. `attested` names the
 * projection tables behind the answer; `live` names what the stamp does
 * not cover. A stamp never implies freshness for anything it does not
 * list.
 */
export interface Stamp {
  serial: number;
  digest: string;
  instance_id: string;
  /** The reader logic version; it moves when the same rows answer differently. */
  derivation: number;
  policy: StampPolicy;
  attested: string[];
  live: LiveWord[];
}

/** Every structured query answers under this envelope. */
export interface QueryEnvelope {
  protocol_version: number;
  operation: string;
  /** Absent only on an answer recorded before the stamp existed. */
  stamp?: Stamp;
  /** The failed freshness step's error, when `policy` is `catch_up_failed`. */
  freshness_error?: string;
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

/**
 * One record one hop away. `relation` names the field that joins the two
 * records: `cites`, `domain_id`, `refines`, `depends_on`, `supersedes`,
 * `spawned_by`, `requirement_ids`, `resolution_ids`, `requirement_id`,
 * `topic_id`, `resolution_id`, `contradicts`, or `links`. `out` means the
 * queried record holds the field; `in` means the neighbour does.
 */
export interface Neighbor {
  relation: string;
  direction: "out" | "in";
  node: GraphNode;
}

export interface NeighborsRequest extends PagedRequest {
  id: string;
  node_type?: NodeType;
  direction?: Direction;
  /** Relation names to follow; every declared relation when omitted. */
  relations?: string[];
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
  /**
   * The working-tree scan stopped at the engine's file count, so the
   * scanned sites are a lower bound. Absent on an answer recorded before
   * the flag existed.
   */
  scan_cut?: boolean;
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
  /**
   * The cut flag of each list; `has_more` is the OR of the four. Absent
   * on an answer recorded before the flags existed.
   */
  implementation_bindings_has_more?: boolean;
  verification_bindings_has_more?: boolean;
  verification_runs_has_more?: boolean;
  reviews_has_more?: boolean;
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
