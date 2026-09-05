import type {
  ApplyResult,
  RequirementDeclaration,
  ResourceKind,
  RuleDeclaration,
  SourceDeclaration,
  TypedSpecDocument,
} from "./protocol.js";
import { STATE_SCHEMA_VERSION } from "./protocol.js";

interface IdentityReceiver {
  assignId(id: string): void;
}

export class DeclarationRegistry {
  readonly sources: SourceDeclaration[] = [];
  readonly requirements: RequirementDeclaration[] = [];
  readonly rules: RuleDeclaration[] = [];
  #handles = new Map<string, IdentityReceiver[]>();
  #dirty = false;

  reset(): void {
    this.sources.length = 0;
    this.requirements.length = 0;
    this.rules.length = 0;
    this.#handles.clear();
    this.#dirty = false;
  }

  addSource(declaration: SourceDeclaration, handle: IdentityReceiver): void {
    this.sources.push(declaration);
    this.#register("source", declaration.key, handle);
  }

  addRequirement(
    declaration: RequirementDeclaration,
    handle: IdentityReceiver,
  ): void {
    this.requirements.push(declaration);
    this.#register("requirement", declaration.key, handle);
  }

  addRule(declaration: RuleDeclaration, handle: IdentityReceiver): void {
    this.rules.push(declaration);
    this.#register("rule", declaration.key, handle, declaration.requirement);
  }

  document(owner: string): TypedSpecDocument {
    return {
      schema_version: STATE_SCHEMA_VERSION,
      spec: "legacy",
      declared_by: owner,
      sources: this.sources,
      requirements: this.requirements,
      rules: this.rules,
    };
  }

  assign(result: ApplyResult): void {
    for (const resource of result.resources) {
      for (const handle of this.#handles.get(
        key(resource.kind, resource.key, resource.parent),
      ) ?? []) {
        handle.assignId(resource.id);
      }
    }
    this.#dirty = false;
  }

  get dirty(): boolean {
    return this.#dirty;
  }

  #register(
    kind: ResourceKind,
    localKey: string,
    handle: IdentityReceiver,
    parent?: string,
  ): void {
    const identity = key(kind, localKey, parent);
    const handles = this.#handles.get(identity) ?? [];
    handles.push(handle);
    this.#handles.set(identity, handles);
    this.#dirty = true;
  }
}

function key(kind: ResourceKind, localKey: string, parent?: string): string {
  return `${kind}\0${parent ?? ""}\0${localKey}`;
}
