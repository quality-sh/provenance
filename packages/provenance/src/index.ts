import { invokeEngine, type EngineSettings } from "./engine.js";
import { fileURLToPath } from "node:url";
import {
  authorSpec,
  type BoundRequirement,
  type BoundRule,
  type BoundSource,
  type RequirementDeclaration,
  type RuleDeclaration,
  type SourceDeclaration,
  type SpecAuthoring,
} from "./bound-spec.js";
import type {
  ApplyResult,
  EvidenceRequest,
  EvidenceResponse,
  GetRequest,
  GetResponse,
  ImpactRequest,
  ImpactResponse,
  NeighborsRequest,
  NeighborsResponse,
  PlanResult,
  ResolveSymbolRequest,
  ResolveSymbolResponse,
  SearchRequest,
  SearchResponse,
  StaleRequest,
  StaleResponse,
  TraceRequest,
  TraceResponse,
  VerificationRun,
} from "./protocol.js";
import { DeclarationRegistry } from "./registry.js";
import {
  fluentRequirement,
  fluentRule,
  fluentSource,
  type FluentRequirement,
  type FluentRule,
  type FluentSource,
  type FluentSpec,
} from "./fluent-spec.js";
import {
  defineSpec as constructSpec,
  specDocument,
  type DeclarationRecord,
  type FinalizedRecord,
  type SpecAuthor,
  type SpecHandle,
} from "./spec.js";
import type { DeclarationAddress } from "./protocol.js";
import { verificationFile } from "./verification-file.js";
import type { VerificationMethod } from "./rules.js";

export type { VerificationMethod } from "./rules.js";

export interface ConfigureOptions {
  engine?: string;
  repository?: string;
  scope?: string;
  owner?: string;
  verificationOwner?: string;
}

export interface SourceOptions {
  id?: string;
  name?: string;
  kind: string;
  url?: string;
  reference?: string;
}

export interface RequirementOptions {
  id?: string;
  statement: string;
  description?: string;
  sources?: SourceHandle[];
}

export interface RuleOptions {
  id?: string;
  statement: string;
  name?: string;
  description?: string;
}

// `file` and `url` both say which file the verification runs in. Pass
// `import.meta` for the whole option, or state one of them yourself.
export interface VerifyOptions {
  method?: VerificationMethod;
  file?: string;
  url?: string;
  symbol?: string;
}

export interface SourceHandle {
  readonly key: string;
  readonly id: string;
}

export interface RequirementHandle {
  readonly key: string;
  readonly id: string;
  rule(key: string, options: RuleOptions): RuleHandle;
}

export interface RuleHandle {
  readonly key: string;
  readonly id: string;
  verify(
    key: string,
    callback: () => unknown | Promise<unknown>,
    options?: VerifyOptions,
  ): Promise<void>;
}

export type {
  AffectedRule,
  ApplyResult,
  Direction,
  EdgeType,
  EvidenceDiffSite,
  EvidenceDiffState,
  EvidenceDiffSummary,
  EvidenceRequest,
  EvidenceResponse,
  EvidenceSiteKind,
  GetRequest,
  GetResponse,
  GraphNode,
  ImpactRequest,
  ImpactResponse,
  Neighbor,
  NeighborsRequest,
  NeighborsResponse,
  NodeType,
  PlanResult,
  QueryEnvelope,
  RequirementReview,
  ResolveSymbolRequest,
  ResolveSymbolResponse,
  ReviewReason,
  RuleEvidence,
  SearchRequest,
  SearchResponse,
  StaleEvidence,
  StaleRequest,
  StaleResponse,
  TraceRequest,
  TraceResponse,
  TracedNode,
  VerificationBinding,
  VerificationRun,
} from "./protocol.js";
export type {
  RequirementHandle as SpecRequirement,
  RuleHandle as SpecRule,
  SourceHandle as SpecSource,
  SpecAuthor,
  SpecHandle,
} from "./spec.js";
export type { FluentRequirement, FluentRule, FluentSource, FluentSpec } from "./fluent-spec.js";
export type {
  BoundRequirement,
  BoundRule,
  BoundSource,
  RequirementDeclaration,
  RuleDeclaration,
  SourceDeclaration,
  SpecAuthoring,
} from "./bound-spec.js";

const registry = new DeclarationRegistry();
const sdkFiles = [
  "./index.js",
  "./spec.js",
  "./bound-spec.js",
  "./fluent-spec.js",
  "./bound-declarations.js",
].map((module) => fileURLToPath(new URL(module, import.meta.url)));
let settings = defaults();

interface SdkSettings {
  engine?: string;
  repository?: string;
  scope: string;
  owner: string;
  verificationOwner: string;
}

export function configure(options: ConfigureOptions): void {
  settings = { ...defaults(), ...options };
  registry.reset();
}

export function source<const Key extends string>(key: Key): FluentSource<Key>;
export function source(key: string, options: SourceOptions): SourceHandle;
export function source(key: string, options?: SourceOptions): SourceHandle | FluentSource {
  if (options === undefined) return fluentSource(key);
  const handle = new DeclaredHandle(key);
  registry.addSource(
    {
      key,
      id: options.id,
      name: options.name ?? key,
      kind: options.kind,
      url: options.url,
      reference: options.reference,
    },
    handle,
  );
  return handle;
}

export function requirement<const Key extends string>(
  key: Key,
): FluentRequirement<Key, readonly [], readonly []>;
export function requirement(key: string, options: RequirementOptions): RequirementHandle;
export function requirement(
  key: string,
  options?: RequirementOptions,
): RequirementHandle | FluentRequirement {
  if (options === undefined) return fluentRequirement(key);
  const handle = new Requirement(key);
  registry.addRequirement(
    {
      key,
      id: options.id,
      statement: options.statement,
      description: options.description,
      sources: (options.sources ?? []).map((source) => source.key),
    },
    handle,
  );
  return handle;
}

export function rule<const Key extends string>(key: Key): FluentRule<Key> {
  return fluentRule(key);
}

export function defineSpec<const Key extends string>(key: Key): SpecAuthoring<Key>;
export function defineSpec<const Declarations extends DeclarationRecord>(
  key: string,
  build: (author: SpecAuthor) => Declarations,
): SpecHandle<FinalizedRecord<Declarations>>;
export function defineSpec<const Declarations extends DeclarationRecord>(
  key: string,
  build?: (author: SpecAuthor) => Declarations,
): SpecHandle<FinalizedRecord<Declarations>> | SpecAuthoring<string> {
  if (build === undefined) return authorSpec(key, verifyDeclaration);
  return constructSpec(key, build, verifyDeclaration);
}

export async function apply(
  spec?: SpecHandle<Readonly<Record<string, unknown>>>,
): Promise<ApplyResult> {
  const result = await invokeEngine<ApplyResult>(
    engineSettings(),
    "apply",
    spec === undefined
      ? registry.document(settings.owner)
      : specDocument(spec, settings.owner),
  );
  if (spec === undefined) {
    registry.assign(result);
  }
  return result;
}

export async function plan(
  spec: SpecHandle<Readonly<Record<string, unknown>>>,
): Promise<PlanResult> {
  return invokeEngine<PlanResult>(
    engineSettings(),
    "plan",
    specDocument(spec, settings.owner),
  );
}

// Structured queries. Each one sends its request to the engine and returns
// the answer unchanged: traversal, filtering, and paging all happen in Rust.

export async function get(request: GetRequest): Promise<GetResponse> {
  return invokeEngine<GetResponse>(engineSettings(), "get", request);
}

export async function search(request: SearchRequest): Promise<SearchResponse> {
  return invokeEngine<SearchResponse>(engineSettings(), "search", request);
}

export async function neighbors(request: NeighborsRequest): Promise<NeighborsResponse> {
  return invokeEngine<NeighborsResponse>(engineSettings(), "neighbors", request);
}

export async function trace(request: TraceRequest): Promise<TraceResponse> {
  return invokeEngine<TraceResponse>(engineSettings(), "trace", request);
}

export async function impact(request: ImpactRequest): Promise<ImpactResponse> {
  return invokeEngine<ImpactResponse>(engineSettings(), "impact", request);
}

export async function evidence(request: EvidenceRequest): Promise<EvidenceResponse> {
  return invokeEngine<EvidenceResponse>(engineSettings(), "evidence", request);
}

export async function stale(request: StaleRequest): Promise<StaleResponse> {
  return invokeEngine<StaleResponse>(engineSettings(), "stale", request);
}

export async function resolveSymbol(
  request: ResolveSymbolRequest,
): Promise<ResolveSymbolResponse> {
  return invokeEngine<ResolveSymbolResponse>(engineSettings(), "resolve-symbol", request);
}

class DeclaredHandle implements SourceHandle {
  #id?: string;

  constructor(readonly key: string) {}

  get id(): string {
    if (this.#id === undefined) {
      throw new Error(
        `Provenance declaration \`${this.key}\` has no canonical id until apply() succeeds`,
      );
    }
    return this.#id;
  }

  assignId(id: string): void {
    this.#id = id;
  }
}

class Requirement extends DeclaredHandle implements RequirementHandle {
  rule(key: string, options: RuleOptions): RuleHandle {
    const handle = new Rule(key);
    registry.addRule(
      {
        key,
        id: options.id,
        requirement: this.key,
        statement: options.statement,
        name: options.name,
        description: options.description,
      },
      handle,
    );
    return handle;
  }
}

class Rule extends DeclaredHandle implements RuleHandle {
  async verify(
    key: string,
    callback: () => unknown | Promise<unknown>,
    options: VerifyOptions = {},
  ): Promise<void> {
    const file = verificationFile(key, options, sdkFiles);
    if (registry.dirty) {
      await apply();
    }
    await runVerification({ rule: this.id }, key, callback, options, file);
  }
}

async function complete(
  run: string,
  status: "passed" | "failed",
  error?: string,
): Promise<void> {
  await invokeEngine<VerificationRun>(engineSettings(), "complete-verification", {
    run,
    status,
    error,
  });
}

async function verifyDeclaration(
  address: DeclarationAddress,
  key: string,
  callback: () => unknown | Promise<unknown>,
  options: VerifyOptions = {},
): Promise<void> {
  const file = verificationFile(key, options, sdkFiles);
  await runVerification(
    { declaration: { declared_by: settings.owner, address } },
    key,
    callback,
    options,
    file,
  );
}

type VerificationTarget =
  | { rule: string }
  | { declaration: { declared_by: string; address: DeclarationAddress } };

async function runVerification(
  target: VerificationTarget,
  key: string,
  callback: () => unknown | Promise<unknown>,
  options: VerifyOptions,
  file: string,
): Promise<void> {
  const run = await invokeEngine<VerificationRun>(
    engineSettings(),
    "begin-verification",
    {
      ...target,
      key,
      method: options.method ?? "examples",
      declared_by: settings.verificationOwner,
      file,
      symbol: options.symbol,
    },
  );
  try {
    await callback();
  } catch (error) {
    try {
      await complete(run.id, "failed", serializeError(error));
    } catch {
      // Preserve the callback as the primary test failure.
    }
    throw error;
  }
  await complete(run.id, "passed");
}

function serializeError(error: unknown): string {
  if (error instanceof Error) {
    return error.stack ?? `${error.name}: ${error.message}`;
  }
  if (typeof error === "string") {
    return error;
  }
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

function defaults(): SdkSettings {
  return {
    engine: process.env.PROVENANCE_BIN,
    repository: process.env.PROVENANCE_REPO,
    scope: process.env.PROVENANCE_SCOPE ?? "default",
    owner: process.env.PROVENANCE_SPEC_OWNER ?? "spec://typescript",
    verificationOwner: process.env.PROVENANCE_VERIFICATION_OWNER ?? "ci://typescript",
  };
}

function engineSettings(): EngineSettings {
  return {
    engine: settings.engine,
    repository: settings.repository,
    scope: settings.scope,
  };
}
