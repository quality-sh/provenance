# `@quality-sh/provenance` TypeScript SDK

This package is an optional typed façade over the Provenance Rust engine. It
does not implement graph semantics or persistence in JavaScript.

```sh
npm install @quality-sh/provenance
npx provenance init --path . --scope default --path-prefix .
```

Those two lines are the whole setup. The install brings the engine for your
platform, `init` writes `.provenance/state/`, and `npx provenance check` reports
`ok` on the project it just created.

Define a spec without touching the engine:

```ts
import { defineSpec, requirement, rule, source } from "@quality-sh/provenance";
import { createShareLink } from "./share-links.js";

export const shareLinks = defineSpec("share-links")
  .requirements(
    requirement("sharing")
      .statement("Users can securely share documentation")
      .from(source("sharing-policy").document("docs/sharing-policy.md"))
      .rules(
        rule("expiry")
          .statement("Share links must expire within 30 days")
          .implementedBy(createShareLink),
      ),
  )
  .build();
```

Each fluent call returns a new immutable declaration. `build()` validates and
finalizes the desired-state document and collects Sources linked with
`Requirement.from(...)`; they do not need to be repeated in `.sources(...)`.
Construction is synchronous and in-memory: importing this module does not write
state or start a process. `build()` returns the frozen, typed objects that tests
import. Rust remains responsible for reconciling them.

Sources can add a display name with `.name(...)`; Requirements can add more
context with `.description(...)`.

## Compatibility helpers

Existing code that uses spec-scoped factories can keep declarations in typed
helpers, including helpers in other files:

```ts
import type {
  RequirementDeclaration,
  RuleDeclaration,
  SpecAuthoring,
} from "@quality-sh/provenance";

export function expiryRule<
  const Spec extends string,
  const RequirementKey extends string,
>(
  requirement: RequirementDeclaration<Spec, RequirementKey>,
): RuleDeclaration<Spec, "expiry", RequirementKey> {
  return requirement.rule("expiry").statement("Share links expire");
}

export function sharing<const Spec extends string>(author: SpecAuthoring<Spec>) {
  return author.requirement("sharing").statement("Shares expire");
}
```

`SourceDeclaration`, `RequirementDeclaration`, and `RuleDeclaration` describe
immutable declarations before `build()`. Their literal spec and Requirement
parameters stop helpers from mixing declarations from different specs.

In this compatibility API, a Rule created through a Requirement belongs to that
Requirement. Equal local keys under different Requirements remain distinct. A
Rule created through the spec context can be shared:

```ts
export const authenticatedExpiry = provenance
  .rule("authenticated-expiry")
  .statement("Authenticated access expires");

const shares = provenance
  .requirement("shares")
  .statement("Shares expire")
  .rules(authenticatedExpiry);
const sessions = provenance
  .requirement("sessions")
  .statement("Sessions expire")
  .rules(authenticatedExpiry);

export default provenance.build(shares, sessions);
```

This emits one Rule with two relationships. Shared versus local identity is
chosen by where the Rule is declared, not inferred from JavaScript object
reuse. Sources linked with `.from(...)` are collected transitively by `build()`.

`implementedBy()` accepts an exported function or class through a direct named
import or a non-computed namespace member. The SDK reads that expression from
the spec source and records the imported module and exported symbol; the runtime
value exists only for TypeScript assignability. It never inspects a function or
class name, body, prototype, or object identity, and it never constructs a class.
Calls, conditionals, computed members, instance methods, anonymous closures,
constructed values, and local functions fail clearly because they do not provide
one durable source identity. Rust checks that the resolved file belongs to the
repository and owns the canonical implementation binding. Production code does
not import Provenance.

Moving a local Rule to a shared declaration, or back, preserves its canonical
ID when Rust finds exactly one owned candidate. If several local Rules could
become the shared Rule, apply fails instead of guessing. An immutable
`.id(existingId)` call can choose the canonical record. Other declarations
omitted from that complete spec are retired, not deleted.

Materialize only this spec at a deliberate entry point:

```ts
import { apply, plan } from "@quality-sh/provenance";
import { shareLinks } from "./provenance.spec.js";

await apply(shareLinks);
```

Preview the same reconciliation without writing canonical state:

```ts
const proposed = await plan(shareLinks);
```

Updated resources include field-level `before` and `after` values. Affected
Rules also list the implementation and verification sites that may need
review. Provenance computes both `plan` and `apply` through the same Rust
reconciliation path.

Each affected Rule carries an `evidence` object saying whether its evidence is
`review_required`, and why. Rewording a Requirement statement puts every Rule
it produces up for review, because the obligation those tests vouch for is no
longer the one that was written down. Each reason names the Requirement, the
field, and its value before and after, so a reviewer can see the wording change
that prompted it. Reasons for a change already applied also carry `changed_at`.

Review required is not the same as stale. Stale means the code holding the
evidence changed and is reported by `provenance stale`. A Requirement wording
change never claims anything about the code. Running the tests for a Rule again
clears its review automatically; the recorded reason stays as history. Ask for
`--format markdown` to read the same explanation as prose.

The result classifies each declaration as `created`, `updated`, `moved`,
`retired`, `conflict`, or `unchanged`. Omission retires only records owned by
that same spec. Their Stable IDs and history remain, active checks ignore them,
and adding the declaration back reactivates the same record. A Rule move
replaces its active owned Requirement edge. Plan returns ownership conflicts as
data; apply refuses them. Hard deletion and ownership transfer are separate and
are not part of this API.

Ask the engine what a change reaches before making it:

```ts
import { evidence, impact, neighbors, trace } from "@quality-sh/provenance";

const reached = await impact({ id: sharing.id });
const behind = await evidence({ rule: expiry.id, base: "origin/main" });
```

`impact` answers the Rules a Source or Requirement reaches, each with the
implementation and verification sites behind it. `evidence` answers one Rule's
`implementation_bindings`, `verification_bindings`, `verification_runs`,
`latest_verification_run`, `review_required` with the `reviews` that raised it,
and `stale`. Stale is read from a diff, so `stale` is null unless the request
names a `base` commit.

`get`, `search`, `neighbors`, `trace`, and `resolveSymbol` answer the rest:
one record by id, records whose text contains a phrase, the records one edge
away, a bounded walk outward, and the Rules bound to a code site. `stale`
answers the evidence sites a commit range disturbed.

```ts
import { get, resolveSymbol, search, stale } from "@quality-sh/provenance";

const around = await neighbors({ id: expiry.id, direction: "in" });
const walked = await trace({ id: retention.id, direction: "out", max_depth: 2 });
```

Every answer opens with `protocol_version` and `operation`. Every request takes
`include_retired`, false by default, and every answer that can hold more than
one record takes `limit`, 50 by default and 200 at most, and reports `limit`
and `has_more`. These functions send their request to the engine and return its
answer unchanged: walking, filtering, and paging all happen in Rust.

Removing `.implementedBy(...)` from an active Rule also retires only that
spec's canonical implementation binding. Plan reports the Rule as updated with
the old implementation and `null` as its field-level before/after values. Adding
the link back reactivates the same binding ID, while changing the imported
symbol updates it in place. Retired bindings remain in canonical exports as
history but no longer make the Rule appear implemented.

A test imports the actual rule handle and runs its callback:

```ts
import { shareLinks } from "./provenance.spec.js";

await shareLinks.requirements.sharing.rules.expiry.verify(
  "share-link-expiry",
  async () => {
    // Exercise ordinary production code with the test runner of your choice.
  },
  import.meta,
);
```

Every verification binding names the file the test runs in. Node and Deno report
the calling file, so the third argument is optional there. Bun does not always
report it: JavaScriptCore takes proper tail calls, so a test written as
`test("...", () => rule.verify(key, callback))` leaves the SDK a stack of SDK
frames and nothing else. Passing `import.meta` states the file on every runtime.
`{ file: import.meta.url }` says the same thing and leaves room for `method` and
`symbol`; under Bun `{ file: import.meta.path }` also works. A call that can
name no file fails before the callback runs and says what to add.

Pointing an owner-local verification key at a different Rule from the same
test file retires the binding that key previously named. Calling it again
reactivates the same binding ID, and moving the key to another file updates it
in place. Retired bindings remain in canonical exports as history but no longer
make the Rule appear verified. Because one run only sees the call sites it ran,
nothing else is retired, and a binding whose test file disappeared is reported
by `provenance stale` instead.

The handle keeps an owner-local declaration address, not a mutable database
ID. Rust resolves that address to the canonical Rule when verification begins.
Calling `verify` before applying the spec fails before the callback runs. A
failed callback is recorded and the original error is rethrown.

The package installs a matching Rust engine through a platform-specific
optional dependency. It does not download a binary from an install script,
compile Rust, or require a global CLI. Before its first operation, the SDK
checks that the engine speaks the supported protocol. Rust then finds the
nearest enclosing Provenance or Git project for each command.

This package owns the `provenance` command and forwards it to that engine
unchanged, so `npx provenance` runs what the install supplied. When the platform
package is absent, after `npm install --omit=optional` or on a host with no
published engine, the command names the missing package and the supported
targets rather than reaching the registry for a command of the same name.

Published targets are macOS arm64/x64, Windows x64, and glibc Linux x64. An
unsupported host fails with the supported target list. These environment
variables override the defaults:

- `PROVENANCE_BIN`: explicit development engine; default packaged engine
- `PROVENANCE_REPO`: explicit repository; default nearest enclosing project
- `PROVENANCE_SCOPE`: scope; default `default`
- `PROVENANCE_SPEC_OWNER`: declaration owner; default `spec://typescript`
- `PROVENANCE_VERIFICATION_OWNER`: evidence producer; default `ci://typescript`

`configure()` provides the same settings in code. The SDK still uses one short
process per command; it does not start a daemon.

Spec-scoped declaration factories, object-options declarations, and the
callback form of `defineSpec()` remain available as compatibility surfaces. The
older object-options API uses a process-local registry and `verify()` applies
pending declarations automatically. New code should prefer the nested fluent
form above plus explicit `apply(spec)` so imports stay free of hidden
persistence.

See `examples/typescript-sdk/` for package-name consumption through a local npm
dependency.
