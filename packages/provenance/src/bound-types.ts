import type {
  FluentRequirement,
  FluentRule,
  FluentSource,
  FluentSpec,
} from "./fluent-spec.js";
import type { SourceKind } from "./protocol.js";
import type { SpecHandle, VerifyOptions } from "./spec.js";
import type { ImplementationTarget } from "./implementation-reference.js";

/** An immutable Source declaration tied to one spec authoring context. */
export interface SourceDeclaration<in out SpecKey extends string, out Key extends string> {
  readonly key: Key;
  id(existingId: string): SourceDeclaration<SpecKey, Key>;
  adoptUnowned(existingId: string): SourceDeclaration<SpecKey, Key>;
  document(reference: string): SourceDeclaration<SpecKey, Key>;
  kind(kind: SourceKind): SourceDeclaration<SpecKey, Key>;
  name(name: string): SourceDeclaration<SpecKey, Key>;
  /** Names the older sources of the same spec this one replaces. */
  supersedes(
    ...sources: readonly SourceDeclaration<SpecKey, string>[]
  ): SourceDeclaration<SpecKey, Key>;
}

/** An immutable Rule declaration, optionally local to one Requirement key. */
export interface RuleDeclaration<
  in out SpecKey extends string,
  out Key extends string,
  in out RequirementKey extends string | undefined,
> {
  readonly key: Key;
  statement(text: string): RuleDeclaration<SpecKey, Key, RequirementKey>;
  id(existingId: string): RuleDeclaration<SpecKey, Key, RequirementKey>;
  adoptUnowned(existingId: string): RuleDeclaration<SpecKey, Key, RequirementKey>;
  implementedBy(target: ImplementationTarget): RuleDeclaration<SpecKey, Key, RequirementKey>;
  /** Names the resolutions, by canonical id, this rule follows from. */
  resolutions(...resolutionIds: readonly string[]): RuleDeclaration<SpecKey, Key, RequirementKey>;
  verify(
    key: string,
    callback: () => unknown | Promise<unknown>,
    options?: VerifyOptions,
  ): Promise<void>;
}

/** An immutable Requirement declaration tied to one spec authoring context. */
export interface RequirementDeclaration<
  in out SpecKey extends string,
  in out Key extends string,
> {
  readonly key: Key;
  id(existingId: string): RequirementDeclaration<SpecKey, Key>;
  adoptUnowned(existingId: string): RequirementDeclaration<SpecKey, Key>;
  statement(text: string): RequirementDeclaration<SpecKey, Key>;
  description(description: string): RequirementDeclaration<SpecKey, Key>;
  from(
    ...sources: readonly SourceDeclaration<SpecKey, string>[]
  ): RequirementDeclaration<SpecKey, Key>;
  rule<const RuleKey extends string>(
    key: RuleKey,
  ): RuleDeclaration<SpecKey, RuleKey, Key>;
  rules(
    ...rules: readonly (
      | RuleDeclaration<SpecKey, string, undefined>
      | RuleDeclaration<SpecKey, string, Key>
    )[]
  ): RequirementDeclaration<SpecKey, Key>;
  /** Names the requirement of the same spec this one refines. */
  refines(parent: RequirementDeclaration<SpecKey, any>): RequirementDeclaration<SpecKey, Key>;
  /** Names the requirements of the same spec this one depends on. */
  dependsOn(
    ...requirements: readonly RequirementDeclaration<SpecKey, any>[]
  ): RequirementDeclaration<SpecKey, Key>;
  /** Names the older requirements of the same spec this one replaces. */
  supersedes(
    ...requirements: readonly RequirementDeclaration<SpecKey, any>[]
  ): RequirementDeclaration<SpecKey, Key>;
  /** Names the resolution, by canonical id, this requirement came out of. */
  spawnedBy(resolutionId: string): RequirementDeclaration<SpecKey, Key>;
}

/** The immutable declaration factory for one literal spec key. */
export interface SpecAuthoring<in out SpecKey extends string> {
  readonly key: SpecKey;
  source<const Key extends string>(key: Key): SourceDeclaration<SpecKey, Key>;
  requirement<const Key extends string>(key: Key): RequirementDeclaration<SpecKey, Key>;
  rule<const Key extends string>(key: Key): RuleDeclaration<SpecKey, Key, undefined>;
  build<const Requirements extends readonly RequirementDeclaration<SpecKey, any>[]>(
    ...requirements: Requirements
  ): SpecHandle<Readonly<Record<string, unknown>>>;
  sources<const Added extends readonly FluentSource[]>(
    ...sources: Added
  ): FluentSpec<Added, readonly []>;
  requirements<
    const Added extends readonly FluentRequirement<
      string,
      readonly FluentRule[],
      readonly FluentSource[]
    >[],
  >(
    ...requirements: Added
  ): FluentSpec<readonly [], Added>;
}

/** @deprecated Prefer SourceDeclaration. */
export type BoundSource<SpecKey extends string, Key extends string> = SourceDeclaration<
  SpecKey,
  Key
>;
/** @deprecated Prefer RequirementDeclaration. */
export type BoundRequirement<SpecKey extends string, Key extends string> = RequirementDeclaration<
  SpecKey,
  Key
>;
/** @deprecated Prefer RuleDeclaration. */
export type BoundRule<
  SpecKey extends string,
  Key extends string,
  RequirementKey extends string | undefined,
> = RuleDeclaration<SpecKey, Key, RequirementKey>;
