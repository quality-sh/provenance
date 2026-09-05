import type {
  DeclarationAddress,
  RequirementDeclaration,
  RuleDeclaration,
  SourceDeclaration,
  TypedSpecDocument,
} from "./protocol.js";
import type { VerificationMethod } from "./rules.js";
import { STATE_SCHEMA_VERSION } from "./protocol.js";

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
  sources?: readonly SourceBuilder[];
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
  readonly kind: "source";
  readonly key: string;
  readonly address: DeclarationAddress;
}

export interface RequirementHandle {
  readonly kind: "requirement";
  readonly key: string;
  readonly address: DeclarationAddress;
  readonly rules: Readonly<Record<string, RuleHandle>>;
}

export interface RuleHandle {
  readonly kind: "rule";
  readonly key: string;
  readonly address: DeclarationAddress;
  verify(
    key: string,
    callback: () => unknown | Promise<unknown>,
    options?: VerifyOptions,
  ): Promise<void>;
}

type RuleVerifier = (
  address: DeclarationAddress,
  key: string,
  callback: () => unknown | Promise<unknown>,
  options?: VerifyOptions,
) => Promise<void>;

class ConstructionSession {
  readonly sources: SourceBuilder[] = [];
  readonly requirements: RequirementBuilder[] = [];
  readonly rules: RuleBuilder[] = [];
  readonly sourceKeys = new Set<string>();
  readonly requirementKeys = new Set<string>();
  active = true;

  constructor(readonly spec: string) {}

  assertActive(): void {
    if (!this.active) {
      throw new Error("the declaration builder has already been finalized");
    }
  }

  close(): void {
    this.active = false;
  }
}

export class SourceBuilder {
  private constructor(
    readonly session: ConstructionSession,
    readonly key: string,
    readonly options: SourceOptions,
  ) {}

  static create(session: ConstructionSession, key: string, options: SourceOptions): SourceBuilder {
    session.assertActive();
    requireKey("source", key);
    if (session.sourceKeys.has(key)) {
      throw new Error(`duplicate source key \`${key}\``);
    }
    session.sourceKeys.add(key);
    const source = new SourceBuilder(session, key, options);
    session.sources.push(source);
    return source;
  }
}

export class RequirementBuilder {
  readonly rules = new Map<string, RuleBuilder>();

  private constructor(
    readonly session: ConstructionSession,
    readonly key: string,
    readonly options: RequirementOptions,
  ) {}

  static create(
    session: ConstructionSession,
    key: string,
    options: RequirementOptions,
  ): RequirementBuilder {
    session.assertActive();
    requireKey("requirement", key);
    if (session.requirementKeys.has(key)) {
      throw new Error(`duplicate requirement key \`${key}\``);
    }
    for (const source of options.sources ?? []) {
      if (source.session !== session) {
        throw new Error(`requirement \`${key}\` references a source from another spec`);
      }
    }
    session.requirementKeys.add(key);
    const requirement = new RequirementBuilder(session, key, options);
    session.requirements.push(requirement);
    return requirement;
  }

  rule(key: string, options: RuleOptions): RuleBuilder {
    this.session.assertActive();
    requireKey("rule", key);
    if (this.rules.has(key)) {
      throw new Error(`duplicate rule key \`${key}\` under requirement \`${this.key}\``);
    }
    const rule = new RuleBuilder(this.session, this, key, options);
    this.rules.set(key, rule);
    this.session.rules.push(rule);
    return rule;
  }
}

export class RuleBuilder {
  constructor(
    readonly session: ConstructionSession,
    readonly requirement: RequirementBuilder,
    readonly key: string,
    readonly options: RuleOptions,
  ) {}
}

export interface SpecAuthor {
  source(key: string, options: SourceOptions): SourceBuilder;
  requirement(key: string, options: RequirementOptions): RequirementBuilder;
}

type DeclarationBuilder = SourceBuilder | RequirementBuilder | RuleBuilder;
export type DeclarationRecord = Readonly<Record<string, DeclarationBuilder>>;
type Finalized<Builder> = Builder extends SourceBuilder
  ? SourceHandle
  : Builder extends RequirementBuilder
    ? RequirementHandle
    : Builder extends RuleBuilder
      ? RuleHandle
      : never;
export type FinalizedRecord<Declarations extends DeclarationRecord> = {
  readonly [Key in keyof Declarations]: Finalized<Declarations[Key]>;
};

export interface SpecHandle<Handles extends Readonly<Record<string, unknown>>> {
  readonly key: string;
  readonly handles: Handles;
}

type DesiredSpecDocument = Omit<TypedSpecDocument, "declared_by">;
const documents = new WeakMap<object, DesiredSpecDocument>();

export function registerSpec<Handles extends Readonly<Record<string, unknown>>>(
  key: string,
  handles: Handles,
  document: DesiredSpecDocument,
): SpecHandle<Handles> {
  const spec = Object.freeze({ key, handles });
  documents.set(spec, document);
  return spec;
}

export function registerSpecProperties<Handles extends Readonly<Record<string, unknown>>>(
  key: string,
  handles: Handles,
  document: DesiredSpecDocument,
): SpecHandle<Handles> & Handles {
  const spec = Object.freeze({ key, handles, ...handles }) as SpecHandle<Handles> & Handles;
  documents.set(spec, document);
  return spec;
}

export function defineSpec<const Declarations extends DeclarationRecord>(
  key: string,
  build: (author: SpecAuthor) => Declarations,
  verify: RuleVerifier,
): SpecHandle<FinalizedRecord<Declarations>> {
  requireKey("spec", key);
  const session = new ConstructionSession(key);
  const declarations = build({
    source: (sourceKey, options) => SourceBuilder.create(session, sourceKey, options),
    requirement: (requirementKey, options) =>
      RequirementBuilder.create(session, requirementKey, options),
  });
  session.close();

  const finalized = finalizeDeclarations(declarations, session, verify);
  return registerSpec(key, finalized, document(session));
}

export function specDocument(
  spec: SpecHandle<Readonly<Record<string, unknown>>>,
  owner: string,
): TypedSpecDocument {
  const document = documents.get(spec);
  if (document === undefined) {
    throw new Error("apply() expected a Spec returned by defineSpec()");
  }
  return { ...document, declared_by: owner };
}

function finalizeDeclarations<Declarations extends DeclarationRecord>(
  declarations: Declarations,
  session: ConstructionSession,
  verify: RuleVerifier,
): FinalizedRecord<Declarations> {
  const handles = new Map<DeclarationBuilder, SourceHandle | RequirementHandle | RuleHandle>();
  for (const source of session.sources) {
    handles.set(
      source,
      Object.freeze({ kind: "source", key: source.key, address: sourceAddress(session, source) }),
    );
  }
  for (const rule of session.rules) {
    const address = ruleAddress(session, rule);
    handles.set(
      rule,
      Object.freeze({
        kind: "rule",
        key: rule.key,
        address,
        verify: (
          key: string,
          callback: () => unknown | Promise<unknown>,
          options?: VerifyOptions,
        ) => verify(address, key, callback, options),
      }),
    );
  }
  for (const requirement of session.requirements) {
    const rules = Object.fromEntries(
      [...requirement.rules].map(([key, rule]) => [key, handles.get(rule) as RuleHandle]),
    );
    handles.set(
      requirement,
      Object.freeze({
        kind: "requirement",
        key: requirement.key,
        address: requirementAddress(session, requirement),
        rules: Object.freeze(rules),
      }),
    );
  }
  return Object.freeze(
    Object.fromEntries(
      Object.entries(declarations).map(([name, declaration]) => {
        if (declaration.session !== session) {
          throw new Error(`export \`${name}\` belongs to another spec`);
        }
        return [name, handles.get(declaration)];
      }),
    ),
  ) as FinalizedRecord<Declarations>;
}

function document(session: ConstructionSession): DesiredSpecDocument {
  const sources = session.sources.map<SourceDeclaration>((source) => ({
    key: source.key,
    id: source.options.id,
    name: source.options.name ?? source.key,
    kind: source.options.kind,
    url: source.options.url,
    reference: source.options.reference,
  }));
  const requirements = session.requirements.map<RequirementDeclaration>((requirement) => ({
    key: requirement.key,
    id: requirement.options.id,
    statement: requirement.options.statement,
    description: requirement.options.description,
    sources: (requirement.options.sources ?? [])
      .map((source) => source.key)
      .sort((left, right) => left.localeCompare(right)),
  }));
  const rules = session.rules.map<RuleDeclaration>((rule) => ({
    key: rule.key,
    id: rule.options.id,
    requirement: rule.requirement.key,
    statement: rule.options.statement,
    name: rule.options.name,
    description: rule.options.description,
  }));
  sources.sort(byKey);
  requirements.sort(byKey);
  rules.sort((left, right) =>
    (left.requirement ?? "").localeCompare(right.requirement ?? "") ||
    left.key.localeCompare(right.key),
  );
  return { schema_version: STATE_SCHEMA_VERSION, spec: session.spec, sources, requirements, rules };
}

function sourceAddress(session: ConstructionSession, source: SourceBuilder): DeclarationAddress {
  return Object.freeze([session.spec, "source", source.key]);
}

function requirementAddress(
  session: ConstructionSession,
  requirement: RequirementBuilder,
): DeclarationAddress {
  return Object.freeze([session.spec, "requirement", requirement.key]);
}

function ruleAddress(session: ConstructionSession, rule: RuleBuilder): DeclarationAddress {
  return Object.freeze([
    session.spec,
    "requirement",
    rule.requirement.key,
    "rule",
    rule.key,
  ]);
}

function requireKey(kind: string, key: string): void {
  if (key.trim() === "") {
    throw new Error(`${kind} key must not be empty`);
  }
}

function byKey(left: { key: string }, right: { key: string }): number {
  return left.key.localeCompare(right.key);
}
