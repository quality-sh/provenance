import type {
  RequirementDeclaration,
  RuleDeclaration,
  TypedSpecDocument,
} from "./protocol.js";
import {
  registerSpecProperties,
  type RequirementHandle,
  type RuleHandle,
  type SourceHandle,
  type SpecHandle,
  type VerifyOptions,
} from "./spec.js";
import { appendByIdentity, requireText, uniqueByKey } from "./fluent-validation.js";
import { FluentRule, FluentSource } from "./fluent-declarations.js";

export { FluentRule, FluentSource } from "./fluent-declarations.js";

type RuleVerifier = (
  address: readonly string[],
  key: string,
  callback: () => unknown | Promise<unknown>,
  options?: VerifyOptions,
) => Promise<void>;

const requirementDescriptions = new WeakMap<object, string>();

/** The relation fields a requirement declaration may set. */
export interface RequirementRelations {
  /** The key of the requirement in the same spec this one refines. */
  readonly refines?: string;
  /** Keys of requirements in the same spec this one depends on. */
  readonly dependsOn: readonly string[];
  /** Keys of older requirements in the same spec this one replaces. */
  readonly supersedes: readonly string[];
  /** The canonical id of the resolution this requirement came out of. */
  readonly spawnedBy?: string;
}

const noRelations: RequirementRelations = Object.freeze({
  dependsOn: Object.freeze([]),
  supersedes: Object.freeze([]),
});

export class FluentRequirement<
  Key extends string = string,
  Rules extends readonly FluentRule[] = readonly [],
  Sources extends readonly FluentSource[] = readonly FluentSource[],
> {
  readonly key: Key;
  readonly text?: string;
  readonly explicitId?: string;
  readonly adoptsUnowned: boolean;
  readonly sourceDeclarations: Sources;
  readonly ruleDeclarations: Rules;
  readonly relations: RequirementRelations;

  constructor(
    key: Key,
    text?: string,
    sources: Sources = [] as unknown as Sources,
    rules: Rules = [] as unknown as Rules,
    description?: string,
    explicitId?: string,
    adoptsUnowned = false,
    relations: RequirementRelations = noRelations,
  ) {
    requireKey("requirement", key);
    this.key = key;
    this.text = text;
    this.explicitId = explicitId;
    this.adoptsUnowned = adoptsUnowned;
    this.sourceDeclarations = Object.freeze([...sources]) as unknown as Sources;
    this.ruleDeclarations = Object.freeze([...rules]) as unknown as Rules;
    this.relations = Object.freeze({
      ...relations,
      dependsOn: Object.freeze([...relations.dependsOn]),
      supersedes: Object.freeze([...relations.supersedes]),
    });
    if (description !== undefined) requirementDescriptions.set(this, description);
    Object.freeze(this);
  }

  statement(text: string): FluentRequirement<Key, Rules, Sources> {
    requireText("requirement statement", text);
    return new FluentRequirement(
      this.key,
      text,
      this.sourceDeclarations,
      this.ruleDeclarations,
      requirementDescriptions.get(this),
      this.explicitId,
      this.adoptsUnowned,
      this.relations,
    );
  }

  id(existingId: string): FluentRequirement<Key, Rules, Sources> {
    requireText("requirement id", existingId);
    return new FluentRequirement(
      this.key,
      this.text,
      this.sourceDeclarations,
      this.ruleDeclarations,
      requirementDescriptions.get(this),
      existingId,
      false,
      this.relations,
    );
  }

  adoptUnowned(existingId: string): FluentRequirement<Key, Rules, Sources> {
    requireText("requirement id", existingId);
    return new FluentRequirement(
      this.key,
      this.text,
      this.sourceDeclarations,
      this.ruleDeclarations,
      requirementDescriptions.get(this),
      existingId,
      true,
      this.relations,
    );
  }

  description(description: string): FluentRequirement<Key, Rules, Sources> {
    requireText("requirement description", description);
    return new FluentRequirement(
      this.key,
      this.text,
      this.sourceDeclarations,
      this.ruleDeclarations,
      description,
      this.explicitId,
      this.adoptsUnowned,
      this.relations,
    );
  }

  from<const Added extends readonly FluentSource[]>(
    ...sources: Added
  ): FluentRequirement<Key, Rules, readonly [...Sources, ...Added]> {
    return new FluentRequirement(
      this.key,
      this.text,
      appendByIdentity(this.sourceDeclarations, sources) as unknown as readonly [
        ...Sources,
        ...Added,
      ],
      this.ruleDeclarations,
      requirementDescriptions.get(this),
      this.explicitId,
      this.adoptsUnowned,
      this.relations,
    );
  }

  rules<const Added extends readonly FluentRule[]>(
    ...rules: Added
  ): FluentRequirement<Key, readonly [...Rules, ...Added], Sources> {
    return new FluentRequirement(
      this.key,
      this.text,
      this.sourceDeclarations,
      appendByIdentity(this.ruleDeclarations, rules) as unknown as readonly [...Rules, ...Added],
      requirementDescriptions.get(this),
      this.explicitId,
      this.adoptsUnowned,
      this.relations,
    );
  }

  // Names the requirement of the same spec this one refines.
  refines(parent: AnyRequirement): FluentRequirement<Key, Rules, Sources> {
    return this.withRelations({ ...this.relations, refines: parent.key });
  }

  // Names the requirements of the same spec this one depends on.
  dependsOn(...requirements: readonly AnyRequirement[]): FluentRequirement<Key, Rules, Sources> {
    return this.withRelations({
      ...this.relations,
      dependsOn: appendByIdentity(
        this.relations.dependsOn,
        requirements.map(({ key }) => key),
      ),
    });
  }

  // Names the older requirements of the same spec this one replaces.
  supersedes(...requirements: readonly AnyRequirement[]): FluentRequirement<Key, Rules, Sources> {
    return this.withRelations({
      ...this.relations,
      supersedes: appendByIdentity(
        this.relations.supersedes,
        requirements.map(({ key }) => key),
      ),
    });
  }

  // Names the resolution, by canonical id, this requirement came out of.
  spawnedBy(resolutionId: string): FluentRequirement<Key, Rules, Sources> {
    requireText("resolution id", resolutionId);
    return this.withRelations({ ...this.relations, spawnedBy: resolutionId });
  }

  private withRelations(relations: RequirementRelations): FluentRequirement<Key, Rules, Sources> {
    return new FluentRequirement(
      this.key,
      this.text,
      this.sourceDeclarations,
      this.ruleDeclarations,
      requirementDescriptions.get(this),
      this.explicitId,
      this.adoptsUnowned,
      relations,
    );
  }
}

type AnyRequirement = FluentRequirement<
  string,
  readonly FluentRule[],
  readonly FluentSource[]
>;

type RuleHandles<Rules extends readonly FluentRule[]> = Readonly<{
  [Declaration in Rules[number] as Declaration["key"]]: RuleHandle;
}>;

type TypedRequirementHandle<Declaration extends AnyRequirement> = Omit<
  RequirementHandle,
  "rules"
> & {
  readonly rules: RuleHandles<Declaration["ruleDeclarations"]>;
};

export type FluentSpecHandles<
  Sources extends readonly FluentSource[],
  Requirements extends readonly AnyRequirement[],
> = Readonly<Record<string, unknown>> & {
  readonly sources: Readonly<{
    [Declaration in
      | Sources[number]
      | Requirements[number]["sourceDeclarations"][number] as Declaration["key"]]: SourceHandle;
  }>;
  readonly requirements: Readonly<{
    [Declaration in Requirements[number] as Declaration["key"]]: TypedRequirementHandle<Declaration>;
  }>;
};

export type FluentBuiltSpec<
  Sources extends readonly FluentSource[],
  Requirements extends readonly AnyRequirement[],
> = SpecHandle<FluentSpecHandles<Sources, Requirements>> &
  FluentSpecHandles<Sources, Requirements>;

export class FluentSpec<
  Sources extends readonly FluentSource[] = readonly [],
  Requirements extends readonly AnyRequirement[] = readonly [],
> {
  readonly key: string;
  readonly sourceDeclarations: readonly FluentSource[];
  readonly requirementDeclarations: readonly AnyRequirement[];
  readonly #verify: RuleVerifier;

  constructor(
    key: string,
    verify: RuleVerifier,
    sources: Sources = [] as unknown as Sources,
    requirements: Requirements = [] as unknown as Requirements,
  ) {
    requireKey("spec", key);
    this.key = key;
    this.#verify = verify;
    this.sourceDeclarations = Object.freeze([...sources]) as unknown as Sources;
    this.requirementDeclarations = Object.freeze([...requirements]) as unknown as Requirements;
    Object.freeze(this);
  }

  sources<const Added extends readonly FluentSource[]>(
    ...sources: Added
  ): FluentSpec<readonly [...Sources, ...Added], Requirements> {
    return new FluentSpec(
      this.key,
      this.#verify,
      appendByIdentity(this.sourceDeclarations, sources) as unknown as readonly [
        ...Sources,
        ...Added,
      ],
      this.requirementDeclarations,
    );
  }

  requirements<const Added extends readonly AnyRequirement[]>(
    ...requirements: Added
  ): FluentSpec<Sources, readonly [...Requirements, ...Added]> {
    return new FluentSpec(
      this.key,
      this.#verify,
      this.sourceDeclarations,
      appendByIdentity(this.requirementDeclarations, requirements) as unknown as readonly [
        ...Requirements,
        ...Added,
      ],
    );
  }

  build(): FluentBuiltSpec<Sources, Requirements> {
    return buildSpec(this, this.#verify);
  }
}

export function fluentSource<const Key extends string>(key: Key): FluentSource<Key> {
  return new FluentSource(key);
}

export function fluentRule<const Key extends string>(key: Key): FluentRule<Key> {
  return new FluentRule(key);
}

export function fluentRequirement<const Key extends string>(
  key: Key,
): FluentRequirement<Key, readonly [], readonly []> {
  return new FluentRequirement(key);
}

export function fluentSpec(key: string, verify: RuleVerifier): FluentSpec {
  return new FluentSpec(key, verify);
}

function buildSpec<
  Sources extends readonly FluentSource[],
  Requirements extends readonly AnyRequirement[],
>(
  spec: FluentSpec<Sources, Requirements>,
  verify: RuleVerifier,
): FluentBuiltSpec<Sources, Requirements> {
  const requirements = uniqueByKey("Requirement", spec.requirementDeclarations);
  const sources = uniqueByKey("Source", [
    ...spec.sourceDeclarations,
    ...requirements.flatMap((requirement) => requirement.sourceDeclarations),
  ]);
  const sourceSet = new Set(sources);
  const memberships = new Map<FluentRule, AnyRequirement[]>();
  const relationOwners = new Map<string, FluentRule>();

  for (const source of sources) {
    if (source.declaration === undefined) {
      throw new Error(`Source declaration \`${source.key}\` has no source type`);
    }
  }
  const requirementKeys = new Set(requirements.map(({ key }) => key));
  for (const requirement of requirements) {
    requireText(`Requirement \`${requirement.key}\` statement`, requirement.text);
    for (const named of relatedKeys(requirement.relations)) {
      if (!requirementKeys.has(named)) {
        throw new Error(
          `Requirement \`${requirement.key}\` names Requirement \`${named}\` not included in the spec`,
        );
      }
    }
    for (const source of requirement.sourceDeclarations) {
      if (!sourceSet.has(source)) {
        throw new Error(
          `Requirement \`${requirement.key}\` references Source \`${source.key}\` not included in the spec`,
        );
      }
    }
    for (const rule of requirement.ruleDeclarations) {
      requireText(`Rule \`${rule.key}\` statement`, rule.text);
      const relation = `${requirement.key}\0${rule.key}`;
      const existing = relationOwners.get(relation);
      if (existing !== undefined && existing !== rule) {
        throw new Error(
          `distinct Rule declarations with key \`${rule.key}\` collide under Requirement \`${requirement.key}\``,
        );
      }
      relationOwners.set(relation, rule);
      const parents = memberships.get(rule);
      if (parents === undefined) memberships.set(rule, [requirement]);
      else parents.push(requirement);
    }
  }

  const addresses = new Map<FluentRule, readonly string[]>();
  const addressOwners = new Map<string, FluentRule>();
  for (const [rule, parents] of memberships) {
    const address = ruleAddress(spec.key, rule.key, parents);
    const serialized = JSON.stringify(address);
    const existing = addressOwners.get(serialized);
    if (existing !== undefined && existing !== rule) {
      throw new Error(
        `distinct Rule declarations with key \`${rule.key}\` resolve to the same declaration address`,
      );
    }
    addressOwners.set(serialized, rule);
    addresses.set(rule, address);
  }

  const ruleHandles = new Map<FluentRule, RuleHandle>();
  for (const [rule, address] of addresses) {
    ruleHandles.set(rule, makeRuleHandle(rule, address, verify));
  }
  const sourceHandles = Object.freeze(
    Object.fromEntries(
      sources.map((source) => [
        source.key,
        Object.freeze({
          kind: "source" as const,
          key: source.key,
          address: Object.freeze([spec.key, "source", source.key]),
        }),
      ]),
    ),
  );
  const requirementHandles = Object.freeze(
    Object.fromEntries(
      requirements.map((requirement) => [
        requirement.key,
        Object.freeze({
          kind: "requirement" as const,
          key: requirement.key,
          address: Object.freeze([spec.key, "requirement", requirement.key]),
          rules: Object.freeze(
            Object.fromEntries(
              requirement.ruleDeclarations.map((rule) => [rule.key, ruleHandles.get(rule)!]),
            ),
          ),
        }),
      ]),
    ),
  );
  const handles = Object.freeze({ sources: sourceHandles, requirements: requirementHandles });
  return registerSpecProperties(
    spec.key,
    handles as FluentSpecHandles<Sources, Requirements>,
    document(spec, sources, requirements, memberships),
  );
}

function document(
  spec: FluentSpec<readonly FluentSource[], readonly AnyRequirement[]>,
  sources: readonly FluentSource[],
  requirements: readonly AnyRequirement[],
  memberships: ReadonlyMap<FluentRule, AnyRequirement[]>,
): Omit<TypedSpecDocument, "declared_by"> {
  const sourceRecords = sources.map((source) => ({
    ...source.declaration!,
    ...listField("supersedes", source.supersededKeys),
  }));
  const requirementRecords = requirements.map<RequirementDeclaration>((requirement) => ({
    key: requirement.key,
    id: requirement.explicitId,
    statement: requirement.text!,
    description: requirementDescriptions.get(requirement),
    sources: requirement.sourceDeclarations.map(({ key }) => key).sort(),
    refines: requirement.relations.refines,
    ...listField("depends_on", requirement.relations.dependsOn),
    ...listField("supersedes", requirement.relations.supersedes),
    spawned_by: requirement.relations.spawnedBy,
  }));
  const ruleRecords = [...memberships].map<RuleDeclaration>(([rule, parents]) => ({
    key: rule.key,
    id: rule.explicitId,
    requirements: parents.map(({ key }) => key).sort(),
    statement: rule.text!,
    implementation: rule.implementation,
    ...listField("resolution_ids", rule.resolutionIds),
  }));
  sourceRecords.sort(byKey);
  requirementRecords.sort(byKey);
  ruleRecords.sort(byKey);
  const adoptUnowned = [
    ...sources
      .filter((source) => source.adoptsUnowned)
      .map((source) => ({ kind: "source" as const, id: source.explicitId! })),
    ...requirements
      .filter((requirement) => requirement.adoptsUnowned)
      .map((requirement) => ({ kind: "requirement" as const, id: requirement.explicitId! })),
    ...[...memberships.keys()]
      .filter((rule) => rule.adoptsUnowned)
      .map((rule) => ({ kind: "rule" as const, id: rule.explicitId! })),
  ];
  return {
    schema_version: 1,
    spec: spec.key,
    ...(adoptUnowned.length === 0 ? {} : { adopt_unowned: adoptUnowned }),
    sources: sourceRecords,
    requirements: requirementRecords,
    rules: ruleRecords,
  };
}

function makeRuleHandle(
  rule: FluentRule,
  address: readonly string[],
  verify: RuleVerifier,
): RuleHandle {
  const frozenAddress = Object.freeze([...address]);
  return Object.freeze({
    kind: "rule" as const,
    key: rule.key,
    address: frozenAddress,
    verify: (
      key: string,
      callback: () => unknown | Promise<unknown>,
      options?: VerifyOptions,
    ) => verify(frozenAddress, key, callback, options),
  });
}

function ruleAddress(
  spec: string,
  key: string,
  parents: readonly AnyRequirement[],
): readonly string[] {
  return parents.length === 1
    ? [spec, "requirement", parents[0]!.key, "rule", key]
    : [spec, "rule", key];
}

function requireKey(kind: string, key: string): void {
  requireText(`${kind} key`, key);
}

/** The keys a requirement's relations name in the same spec. */
function relatedKeys(relations: RequirementRelations): string[] {
  return [
    ...(relations.refines === undefined ? [] : [relations.refines]),
    ...relations.dependsOn,
    ...relations.supersedes,
  ];
}

/** A list field travels only when something is in it; an omitted list
 *  leaves the canonical record untouched. */
function listField<Name extends string>(
  name: Name,
  values: readonly string[],
): { [Key in Name]?: string[] } {
  return values.length === 0
    ? {}
    : ({ [name]: [...values].sort() } as { [Key in Name]?: string[] });
}

function byKey(left: { key: string }, right: { key: string }): number {
  return left.key.localeCompare(right.key);
}
