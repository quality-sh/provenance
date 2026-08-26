import { fileURLToPath } from "node:url";

import {
  captureImplementationReference,
  type ImplementationTarget,
} from "./implementation-reference.js";
import type {
  ImplementationDeclaration,
  SourceDeclaration,
  SourceKind,
} from "./protocol.js";
import type { VerifyOptions } from "./spec.js";
import type {
  RuleDeclaration as PublicRule,
  SourceDeclaration as PublicSource,
} from "./bound-types.js";

export type RuleVerifier = (
  address: readonly string[],
  key: string,
  callback: () => unknown | Promise<unknown>,
  options?: VerifyOptions,
) => Promise<void>;

export interface AuthoringContext {
  readonly key: string;
  readonly token: object;
  readonly verify: RuleVerifier;
}

export interface SourceState {
  readonly context: AuthoringContext;
  readonly lineage: object;
  readonly key: string;
  readonly explicitId?: string;
  readonly adoptsUnowned?: boolean;
  readonly name?: string;
  readonly declaration?: SourceDeclaration;
}

export interface RequirementState {
  readonly context: AuthoringContext;
  readonly lineage: object;
  readonly key: string;
  readonly explicitId?: string;
  readonly adoptsUnowned?: boolean;
  readonly text?: string;
  readonly description?: string;
  readonly sources: readonly object[];
  readonly rules: readonly object[];
}

export interface RuleState {
  readonly context: AuthoringContext;
  readonly lineage: object;
  readonly key: string;
  readonly address: readonly string[];
  readonly owner?: { readonly key: string; readonly lineage: object };
  readonly text?: string;
  readonly explicitId?: string;
  readonly adoptsUnowned?: boolean;
  readonly implementation?: ImplementationDeclaration;
}

const declarationState: unique symbol = Symbol("Provenance declaration state");
const moduleFile = fileURLToPath(import.meta.url);
declare const specAffinity: unique symbol;
declare const ownerAffinity: unique symbol;

type Invariant<Value> = {
  readonly input: (value: Value) => void;
  readonly output: Value;
};

interface Stateful<State> {
  readonly [declarationState]: State;
}

export class BoundSource<SpecKey extends string, Key extends string> {
  declare readonly [specAffinity]: Invariant<SpecKey>;
  readonly key: Key;

  private constructor(state: SourceState) {
    this.key = state.key as Key;
    setState(this, state);
    Object.freeze(this);
  }

  static create<SpecKey extends string, Key extends string>(
    context: AuthoringContext,
    key: Key,
  ): BoundSource<SpecKey, Key> {
    requireKey("source", key);
    return new BoundSource({ context, lineage: Object.freeze({}), key });
  }

  // The short form of `kind("document")` that also gives the reference.
  document(reference: string): BoundSource<SpecKey, Key> {
    requireText("document reference", reference);
    return this.withKind("document", reference);
  }

  // Selects the canonical source type. It adds no optional URL or
  // reference metadata.
  kind(kind: SourceKind): BoundSource<SpecKey, Key> {
    return this.withKind(kind, sourceState(this).declaration?.reference);
  }

  private withKind(kind: SourceKind, reference?: string): BoundSource<SpecKey, Key> {
    const state = sourceState(this);
    return new BoundSource({
      ...state,
      declaration: Object.freeze({
        key: state.key,
        id: state.explicitId,
        name: state.name ?? state.key,
        kind,
        reference,
      }),
    });
  }

  id(existingId: string): BoundSource<SpecKey, Key> {
    requireText("source id", existingId);
    const state = sourceState(this);
    return new BoundSource({
      ...state,
      explicitId: existingId,
      adoptsUnowned: false,
      declaration:
        state.declaration === undefined
          ? undefined
          : Object.freeze({ ...state.declaration, id: existingId }),
    });
  }

  adoptUnowned(existingId: string): BoundSource<SpecKey, Key> {
    return this.id(existingId).copyAdoption();
  }

  name(name: string): BoundSource<SpecKey, Key> {
    requireText("source name", name);
    const state = sourceState(this);
    return new BoundSource({
      ...state,
      name,
      declaration:
        state.declaration === undefined
          ? undefined
          : Object.freeze({ ...state.declaration, name }),
    });
  }

  private copyAdoption(): BoundSource<SpecKey, Key> {
    return new BoundSource({ ...sourceState(this), adoptsUnowned: true });
  }
}

export class BoundRule<
  SpecKey extends string,
  Key extends string,
  Owner extends string | undefined,
> {
  declare readonly [specAffinity]: Invariant<SpecKey>;
  declare readonly [ownerAffinity]: Invariant<Owner>;
  readonly key: Key;

  private constructor(state: RuleState) {
    this.key = state.key as Key;
    setState(this, freezeRuleState(state));
    Object.freeze(this);
  }

  static shared<SpecKey extends string, Key extends string>(
    context: AuthoringContext,
    key: Key,
  ): BoundRule<SpecKey, Key, undefined> {
    requireKey("rule", key);
    return new BoundRule({
      context,
      lineage: Object.freeze({}),
      key,
      address: [context.key, "rule", key],
    });
  }

  static local<SpecKey extends string, Key extends string, Owner extends string>(
    context: AuthoringContext,
    owner: { readonly key: Owner; readonly lineage: object },
    key: Key,
  ): BoundRule<SpecKey, Key, Owner> {
    requireKey("rule", key);
    return new BoundRule({
      context,
      lineage: Object.freeze({}),
      key,
      address: [context.key, "requirement", owner.key, "rule", key],
      owner,
    });
  }

  statement(text: string): BoundRule<SpecKey, Key, Owner> {
    requireText("rule statement", text);
    return this.copy({ text });
  }

  id(existingId: string): BoundRule<SpecKey, Key, Owner> {
    requireText("rule id", existingId);
    return this.copy({ explicitId: existingId, adoptsUnowned: false });
  }

  adoptUnowned(existingId: string): BoundRule<SpecKey, Key, Owner> {
    requireText("rule id", existingId);
    return this.copy({ explicitId: existingId, adoptsUnowned: true });
  }

  implementedBy(_target: ImplementationTarget): BoundRule<SpecKey, Key, Owner> {
    return this.copy({ implementation: captureImplementationReference([moduleFile]) });
  }

  verify(
    key: string,
    callback: () => unknown | Promise<unknown>,
    options?: VerifyOptions,
  ): Promise<void> {
    const state = ruleState(this);
    return state.context.verify(state.address, key, callback, options);
  }

  private copy(change: Partial<RuleState>): BoundRule<SpecKey, Key, Owner> {
    return new BoundRule({ ...ruleState(this), ...change });
  }
}

export class BoundRequirement<SpecKey extends string, Key extends string> {
  declare readonly [specAffinity]: Invariant<SpecKey>;
  readonly key: Key;

  private constructor(state: RequirementState) {
    this.key = state.key as Key;
    setState(this, freezeRequirementState(state));
    Object.freeze(this);
  }

  static create<SpecKey extends string, Key extends string>(
    context: AuthoringContext,
    key: Key,
  ): BoundRequirement<SpecKey, Key> {
    requireKey("requirement", key);
    return new BoundRequirement({
      context,
      lineage: Object.freeze({}),
      key,
      sources: [],
      rules: [],
    });
  }

  statement(text: string): BoundRequirement<SpecKey, Key> {
    requireText("requirement statement", text);
    return this.copy({ text });
  }

  id(existingId: string): BoundRequirement<SpecKey, Key> {
    requireText("requirement id", existingId);
    return this.copy({ explicitId: existingId, adoptsUnowned: false });
  }

  adoptUnowned(existingId: string): BoundRequirement<SpecKey, Key> {
    requireText("requirement id", existingId);
    return this.copy({ explicitId: existingId, adoptsUnowned: true });
  }

  description(description: string): BoundRequirement<SpecKey, Key> {
    requireText("requirement description", description);
    return this.copy({ description });
  }

  from(...sources: readonly PublicSource<SpecKey, string>[]): BoundRequirement<SpecKey, Key> {
    const state = requirementState(this);
    for (const source of sources) assertContext("Source", sourceState(source), state.context);
    return this.copy({ sources: appendSnapshots("Source", state.sources, sources, sourceState) });
  }

  rule<const RuleKey extends string>(key: RuleKey): BoundRule<SpecKey, RuleKey, Key> {
    const state = requirementState(this);
    return BoundRule.local(
      state.context,
      Object.freeze({ key: this.key, lineage: state.lineage }),
      key,
    );
  }

  rules(
    ...rules: readonly (
      | PublicRule<SpecKey, string, undefined>
      | PublicRule<SpecKey, string, Key>
    )[]
  ): BoundRequirement<SpecKey, Key> {
    const state = requirementState(this);
    for (const rule of rules) validateRuleOwner(state, ruleState(rule));
    return this.copy({ rules: appendSnapshots("Rule", state.rules, rules, ruleState) });
  }

  private copy(change: Partial<RequirementState>): BoundRequirement<SpecKey, Key> {
    return new BoundRequirement({ ...requirementState(this), ...change });
  }
}

export function createAuthoringContext(key: string, verify: RuleVerifier): AuthoringContext {
  requireKey("spec", key);
  return Object.freeze({ key, token: Object.freeze({}), verify });
}

export function sourceState(value: object): SourceState {
  return requiredState("Source", value);
}

export function requirementState(value: object): RequirementState {
  return requiredState("Requirement", value);
}

export function ruleState(value: object): RuleState {
  return requiredState("Rule", value);
}

export function assertContext(
  kind: string,
  state: { context: AuthoringContext; key: string },
  expected: AuthoringContext,
): void {
  if (state.context.token !== expected.token) {
    throw new Error(`${kind} \`${state.key}\` belongs to another spec authoring context`);
  }
}

export function requireText(field: string, value: string | undefined): asserts value is string {
  if (value === undefined || value.trim() === "") throw new Error(`${field} must not be empty`);
}

function validateRuleOwner(requirement: RequirementState, rule: RuleState): void {
  assertContext("Rule", rule, requirement.context);
  if (rule.owner?.key !== undefined && rule.owner.key !== requirement.key) {
    throw new Error(
      `local Rule \`${rule.key}\` belongs to Requirement \`${rule.owner.key}\`, not \`${requirement.key}\``,
    );
  }
  if (rule.owner !== undefined && rule.owner.lineage !== requirement.lineage) {
    throw new Error(
      `local Rule \`${rule.key}\` belongs to another \`${requirement.key}\` Requirement declaration`,
    );
  }
}

function appendSnapshots<T extends object, State extends { lineage: object; key: string }>(
  kind: string,
  existing: readonly object[],
  added: readonly T[],
  stateOf: (value: object) => State,
): readonly object[] {
  const result = [...existing];
  for (const value of added) {
    if (result.includes(value)) continue;
    const state = stateOf(value);
    const prior = result.find((candidate) => stateOf(candidate).lineage === state.lineage);
    if (prior !== undefined) {
      throw new Error(`${kind} \`${state.key}\` is included through more than one immutable snapshot`);
    }
    result.push(value);
  }
  return Object.freeze(result);
}

function freezeRequirementState(state: RequirementState): RequirementState {
  return Object.freeze({
    ...state,
    sources: Object.freeze([...state.sources]),
    rules: Object.freeze([...state.rules]),
  });
}

function freezeRuleState(state: RuleState): RuleState {
  return Object.freeze({
    ...state,
    address: Object.freeze([...state.address]),
    owner: state.owner === undefined ? undefined : Object.freeze({ ...state.owner }),
  });
}

function setState<State>(target: object, state: State): void {
  Object.defineProperty(target, declarationState, {
    value: Object.freeze(state),
    enumerable: false,
    configurable: false,
    writable: false,
  });
}

function requiredState<State>(kind: string, value: object): State {
  const state = (value as Partial<Stateful<State>>)[declarationState];
  if (state === undefined) throw new Error(`${kind} was not created by Provenance`);
  return state;
}

function requireKey(kind: string, key: string): void {
  requireText(`${kind} key`, key);
}
