import type {
  RequirementDeclaration,
  RuleDeclaration,
  SourceDeclaration,
  TypedSpecDocument,
} from "./protocol.js";
import {
  assertContext,
  requirementState,
  requireText,
  ruleState,
  sourceState,
  type AuthoringContext,
  type RequirementState,
  type RuleState,
  type SourceState,
} from "./bound-declarations.js";
import { registerSpec, type SpecHandle } from "./spec.js";

interface Collected<State> {
  readonly value: object;
  readonly state: State;
}

interface CollectedRule extends Collected<RuleState> {
  readonly parents: Set<string>;
}

export function buildBoundSpec(
  context: AuthoringContext,
  roots: readonly object[],
): SpecHandle<Readonly<Record<string, unknown>>> {
  const requirements = uniqueRoots(context, roots);
  const sources = new Map<string, Collected<SourceState>>();
  const rules = new Map<string, CollectedRule>();
  for (const requirement of requirements) {
    const state = requirementState(requirement);
    requireText(`Requirement \`${state.key}\` statement`, state.text);
    for (const source of state.sources) collectSource(context, sources, source);
    collectRules(context, rules, state);
  }
  const document = compileDocument(context.key, requirements, sources, rules);
  return registerSpec(context.key, Object.freeze({}), document);
}

function uniqueRoots(context: AuthoringContext, roots: readonly object[]): object[] {
  const values = new Map<string, Collected<RequirementState>>();
  for (const value of roots) {
    const state = requirementState(value);
    assertContext("Requirement", state, context);
    const existing = values.get(state.key);
    if (existing?.value === value) continue;
    if (existing !== undefined) {
      throw new Error(
        `distinct Requirement declarations claim address \`${context.key} / requirement / ${state.key}\``,
      );
    }
    values.set(state.key, { value, state });
  }
  return [...values.values()].map(({ value }) => value);
}

function collectSource(
  context: AuthoringContext,
  collected: Map<string, Collected<SourceState>>,
  value: object,
): void {
  const state = sourceState(value);
  assertContext("Source", state, context);
  if (state.declaration === undefined) {
    throw new Error(`Source declaration \`${state.key}\` has no source type`);
  }
  const existing = collected.get(state.key);
  if (existing?.value === value) return;
  if (existing !== undefined) {
    throw new Error(
      `distinct Source declarations claim address \`${context.key} / source / ${state.key}\``,
    );
  }
  collected.set(state.key, { value, state });
}

function collectRules(
  context: AuthoringContext,
  collected: Map<string, CollectedRule>,
  requirement: RequirementState,
): void {
  const relations = new Map<string, RuleState>();
  for (const value of requirement.rules) {
    const state = ruleState(value);
    const existingRelation = relations.get(state.key);
    if (existingRelation !== undefined && existingRelation.lineage !== state.lineage) {
      throw new Error(
        `distinct Rule declarations with key \`${state.key}\` collide under Requirement \`${requirement.key}\``,
      );
    }
    relations.set(state.key, state);
    collectRule(context, collected, requirement, value, state);
  }
}

function collectRule(
  context: AuthoringContext,
  collected: Map<string, CollectedRule>,
  requirement: RequirementState,
  value: object,
  state: RuleState,
): void {
  assertContext("Rule", state, context);
  requireText(`Rule \`${state.key}\` statement`, state.text);
  const addressKey = JSON.stringify(state.address);
  const existing = collected.get(addressKey);
  if (existing === undefined) {
    collected.set(addressKey, { value, state, parents: new Set([requirement.key]) });
    return;
  }
  if (existing.value !== value) {
    throw new Error(`distinct Rule declarations claim address \`${state.address.join(" / ")}\``);
  }
  existing.parents.add(requirement.key);
}

function compileDocument(
  spec: string,
  requirements: readonly object[],
  sources: ReadonlyMap<string, Collected<SourceState>>,
  rules: ReadonlyMap<string, CollectedRule>,
): Omit<TypedSpecDocument, "declared_by"> {
  const sourceRecords = [...sources.values()].map(({ state }) => state.declaration!);
  const requirementRecords = requirements.map<RequirementDeclaration>((value) => {
    const state = requirementState(value);
    return {
      key: state.key,
      id: state.explicitId,
      statement: state.text!,
      description: state.description,
      sources: state.sources.map((source) => sourceState(source).key).sort(),
    };
  });
  const ruleRecords = [...rules.values()].map<RuleDeclaration>(({ state, parents }) => ({
    key: state.key,
    id: state.explicitId,
    address: state.address,
    requirements: [...parents].sort(),
    statement: state.text!,
    implementation: state.implementation,
  }));
  sourceRecords.sort(byKey);
  requirementRecords.sort(byKey);
  ruleRecords.sort((left, right) =>
    JSON.stringify(left.address).localeCompare(JSON.stringify(right.address)),
  );
  const adoptUnowned = [
    ...[...sources.values()]
      .filter(({ state }) => state.adoptsUnowned)
      .map(({ state }) => ({ kind: "source" as const, id: state.explicitId! })),
    ...requirements
      .map(requirementState)
      .filter((state) => state.adoptsUnowned)
      .map((state) => ({ kind: "requirement" as const, id: state.explicitId! })),
    ...[...rules.values()]
      .filter(({ state }) => state.adoptsUnowned)
      .map(({ state }) => ({ kind: "rule" as const, id: state.explicitId! })),
  ];
  return {
    schema_version: 1,
    spec,
    ...(adoptUnowned.length === 0 ? {} : { adopt_unowned: adoptUnowned }),
    sources: sourceRecords,
    requirements: requirementRecords,
    rules: ruleRecords,
  };
}

function byKey(left: { key: string }, right: { key: string }): number {
  return left.key.localeCompare(right.key);
}
