import { fileURLToPath } from "node:url";

import {
  captureImplementationReference,
  type ImplementationTarget,
} from "./implementation-reference.js";
import type { ImplementationDeclaration, SourceDeclaration, SourceKind } from "./protocol.js";
import { requireText } from "./fluent-validation.js";

const moduleFile = fileURLToPath(import.meta.url);
const sourceNames = new WeakMap<object, string>();

export class FluentSource<Key extends string = string> {
  readonly key: Key;
  readonly declaration?: SourceDeclaration;
  readonly explicitId?: string;
  readonly adoptsUnowned: boolean;

  constructor(
    key: Key,
    declaration?: SourceDeclaration,
    displayName?: string,
    explicitId?: string,
    adoptsUnowned = false,
  ) {
    requireText("source key", key);
    this.key = key;
    this.declaration = declaration === undefined ? undefined : Object.freeze({ ...declaration });
    this.explicitId = explicitId;
    this.adoptsUnowned = adoptsUnowned;
    if (displayName !== undefined) sourceNames.set(this, displayName);
    Object.freeze(this);
  }

  id(existingId: string): FluentSource<Key> {
    requireText("source id", existingId);
    return this.copy(existingId, false);
  }

  adoptUnowned(existingId: string): FluentSource<Key> {
    requireText("source id", existingId);
    return this.copy(existingId, true);
  }

  // The short form of `kind("document")` that also gives the reference.
  document(reference: string): FluentSource<Key> {
    requireText("document reference", reference);
    return this.withKind("document", reference);
  }

  // Selects the canonical source type. It adds no optional URL or
  // reference metadata.
  kind(kind: SourceKind): FluentSource<Key> {
    return this.withKind(kind, this.declaration?.reference);
  }

  private withKind(kind: SourceKind, reference?: string): FluentSource<Key> {
    const displayName = sourceNames.get(this);
    return new FluentSource(
      this.key,
      {
        key: this.key,
        id: this.explicitId,
        name: displayName ?? this.key,
        kind,
        reference,
      },
      displayName,
      this.explicitId,
      this.adoptsUnowned,
    );
  }

  name(name: string): FluentSource<Key> {
    requireText("source name", name);
    return new FluentSource(
      this.key,
      this.declaration === undefined ? undefined : { ...this.declaration, name },
      name,
      this.explicitId,
      this.adoptsUnowned,
    );
  }

  private copy(explicitId: string, adoptsUnowned: boolean): FluentSource<Key> {
    return new FluentSource(
      this.key,
      this.declaration === undefined ? undefined : { ...this.declaration, id: explicitId },
      sourceNames.get(this),
      explicitId,
      adoptsUnowned,
    );
  }
}

export class FluentRule<Key extends string = string> {
  readonly key: Key;
  readonly text?: string;
  readonly explicitId?: string;
  readonly implementation?: ImplementationDeclaration;
  readonly adoptsUnowned: boolean;

  constructor(
    key: Key,
    text?: string,
    explicitId?: string,
    implementation?: ImplementationDeclaration,
    adoptsUnowned = false,
  ) {
    requireText("rule key", key);
    this.key = key;
    this.text = text;
    this.explicitId = explicitId;
    this.implementation = implementation;
    this.adoptsUnowned = adoptsUnowned;
    Object.freeze(this);
  }

  statement(text: string): FluentRule<Key> {
    requireText("rule statement", text);
    return new FluentRule(
      this.key,
      text,
      this.explicitId,
      this.implementation,
      this.adoptsUnowned,
    );
  }

  id(existingId: string): FluentRule<Key> {
    requireText("rule id", existingId);
    return new FluentRule(this.key, this.text, existingId, this.implementation);
  }

  adoptUnowned(existingId: string): FluentRule<Key> {
    requireText("rule id", existingId);
    return new FluentRule(this.key, this.text, existingId, this.implementation, true);
  }

  implementedBy(_target: ImplementationTarget): FluentRule<Key> {
    const implementation = captureImplementationReference([moduleFile]);
    return new FluentRule(
      this.key,
      this.text,
      this.explicitId,
      implementation,
      this.adoptsUnowned,
    );
  }
}
