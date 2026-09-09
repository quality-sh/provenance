import { connection, context, defaults, type ConfigureOptions } from "./settings.js";
import { documentPaths } from "./document-paths.js";
import { portableFile } from "./portable-file.js";
import { runVerification, type VerifyOptions } from "./verification.js";
export type { ConfigureOptions } from "./settings.js";
export type { VerifyOptions } from "./verification.js";
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
  StampPolicy,
  StaleRequest,
  StaleResponse,
  TraceRequest,
  TraceResponse,
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

export type { VerificationMethod } from "./rules.js";

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
  LiveWord,
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
  SourceKind,
  StaleEvidence,
  StaleRequest,
  StaleResponse,
  Stamp,
  StampPolicy,
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

export function configure(options: ConfigureOptions): void {
  if ("engine" in options || "repository" in options) {
    throw new Error("Use endpoint, bearer, repositoryId, and localRoot; subprocess SDK configuration is no longer supported");
  }
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
  const selected = settings;
  const document = spec === undefined ? registry.document(selected.owner) : specDocument(spec, selected.owner);
  const result = await (await connection(selected)).apply({
    context: context(selected), request: documentPaths(document, selected.localRoot),
  });
  if (spec === undefined) {
    registry.assign(result);
  }
  return result;
}

export async function plan(
  spec: SpecHandle<Readonly<Record<string, unknown>>>,
): Promise<PlanResult> {
  const selected = settings;
  return (await connection(selected)).plan({
    context: context(selected), request: documentPaths(specDocument(spec, selected.owner), selected.localRoot),
  });
}

export interface QueryOptions {
  freshness?: Exclude<StampPolicy, "catch_up_failed">;
}

export async function get(request: GetRequest, options?: QueryOptions): Promise<GetResponse> {
  const selected = settings;
  return (await connection(selected)).get({ context: { ...context(selected), freshness: options?.freshness }, request });
}

export async function search(request: SearchRequest, options?: QueryOptions): Promise<SearchResponse> {
  const selected = settings;
  return (await connection(selected)).search({ context: { ...context(selected), freshness: options?.freshness }, request });
}

export async function neighbors(request: NeighborsRequest, options?: QueryOptions): Promise<NeighborsResponse> {
  const selected = settings;
  return (await connection(selected)).neighbors({ context: { ...context(selected), freshness: options?.freshness }, request });
}

export async function trace(request: TraceRequest, options?: QueryOptions): Promise<TraceResponse> {
  const selected = settings;
  return (await connection(selected)).trace({ context: { ...context(selected), freshness: options?.freshness }, request });
}

export async function impact(request: ImpactRequest, options?: QueryOptions): Promise<ImpactResponse> {
  const selected = settings;
  return (await connection(selected)).impact({ context: { ...context(selected), freshness: options?.freshness }, request });
}

export async function evidence(request: EvidenceRequest, options?: QueryOptions): Promise<EvidenceResponse> {
  const selected = settings;
  return (await connection(selected)).evidence({ context: { ...context(selected), freshness: options?.freshness }, request });
}

export async function stale(request: StaleRequest, options?: QueryOptions): Promise<StaleResponse> {
  const selected = settings;
  return (await connection(selected)).stale({ context: { ...context(selected), freshness: options?.freshness }, request });
}

export async function resolveSymbol(
  request: ResolveSymbolRequest,
  options?: QueryOptions,
): Promise<ResolveSymbolResponse> {
  const selected = settings;
  return (await connection(selected)).resolveSymbol({ context: { ...context(selected), freshness: options?.freshness }, request: { ...request, file: portableFile(request.file, selected.localRoot) } });
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
    await runVerification(settings, { rule: this.id }, key, callback, options, file);
  }
}

async function verifyDeclaration(
  address: DeclarationAddress,
  key: string,
  callback: () => unknown | Promise<unknown>,
  options: VerifyOptions = {},
): Promise<void> {
  const file = verificationFile(key, options, sdkFiles);
  await runVerification(
    settings,
    { declaration: { declared_by: settings.owner, address } },
    key,
    callback,
    options,
    file,
  );
}
