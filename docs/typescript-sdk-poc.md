# TypeScript SDK proof of concept

## Outcome

The typed surface works with a small façade and a one-shot child-process
protocol. TypeScript owns pure declaration construction, immutable handles,
callback execution, and error serialization. The Rust engine assigns IDs,
validates and reconciles records, writes canonical state and graph edges, and
stores verification outcomes.

The package interface is:

```ts
import { defineSpec, requirement, rule, source } from "@quality-sh/provenance";

export const shareLinks = defineSpec("share-links")
  .requirements(
    requirement("sharing")
      .statement("Users can securely share documentation")
      .from(source("sharing-policy").document("docs/sharing-policy.md"))
      .rules(
        rule("expiry").statement("Share links must expire within 30 days"),
      ),
  )
  .build();
```

An explicit entry point calls `apply(shareLinks)`. Importing the declaration
module only constructs and freezes values in memory. Tests import `shareLinks`
and call
`shareLinks.requirements.sharing.rules.expiry.verify("share-link-expiry", callback)`.
The local key gives the durable Verification binding a stable identity; the
Rule itself remains a real typed reference. The callback never crosses the
process seam. Node runs it between Rust-backed begin and complete commands. On
failure, the SDK sends a serialized error and rethrows the exact value caught
from the callback.

## Process protocol

The SDK launches the CLI for each operation and exchanges one JSON document on
stdin/stdout:

- `provenance sdk info` reports the engine, protocol, state schema, and resolved
  project root. The SDK uses it to reject an incompatible engine before sending
  declarations or evidence.
- `provenance sdk apply` reconciles one complete declaration document.
- `provenance sdk plan` previews the same reconciliation without publishing it.
- `provenance sdk begin-verification` checks the rule and creates a running
  evidence record.
- `provenance sdk complete-verification` records passed or failed.
- `provenance sdk verification-runs` queries that evidence, optionally by rule.
- `provenance sdk get`, `search`, `neighbors`, `trace`, `impact`, `evidence`,
  `stale`, and `resolve-symbol` answer structured questions about the graph.
  Each is one named operation with a bounded answer, described in
  [`cli.md`](cli.md). The TypeScript functions over them add no traversal or
  filtering of their own.

No daemon, socket, native addon, FFI object graph, or callback bridge is used
in this POC. Each verification uses two short-lived Rust processes. Published
SDK packages resolve a platform-specific optional dependency containing the
Rust engine. Installation runs no binary download or Rust compilation.
`PROVENANCE_BIN` remains an explicit development override.

An explicit `--repo` / `PROVENANCE_REPO` setting wins. Otherwise the engine
walks upward from the working directory and selects the nearest initialized
Provenance project or Git root. This keeps project discovery in Rust so later
language SDKs share the same behaviour.

## Identity and ownership

Each declaration has two identities:

- a structured, owner-local declaration address used by typed handles;
- a canonical Provenance Stable ID assigned or accepted by Rust.

The address includes the spec and hierarchy. Distinct Rules created through
`sharing.rule("expiry")` and `sessions.rule("expiry")` remain separate, as do
equal top-level keys in different specs. A Rule created through
`provenance.rule("expiry")` has an explicitly spec-scoped address and can refine
several Requirements. Shared identity is not inferred from object reuse.
Declaration keys are not TypeScript variable names. Renaming:

```ts
const provenance = defineSpec("share-links");
const sharing = provenance.requirement("sharing").statement(...);
const expiry = sharing.rule("expiry").statement(...);
```

to:

```ts
const provenance = defineSpec("share-links");
const sharing = provenance.requirement("sharing").statement(...);
const shareLinkExpiry = sharing.rule("expiry").statement(...);
```

leaves both the declaration address and canonical ID unchanged. On reapply,
Rust first resolves the owner and address to the persisted Stable ID. A new
address receives a deterministic implicit ID. Moving one owned local Rule to a
shared address, or one shared Rule to a local address, keeps its Stable ID when
there is exactly one matching candidate. If several local Rules could be
merged, Rust rejects the guess; `rule(key).id(existingId)` selects which
canonical Rule receives the new address. Other owned declarations omitted from
the complete document are retired, not deleted.
Immutable handles do not cache IDs;
`apply` returns them and Rust resolves later verification by owner and address.

Sources, requirements, and rules carry optional `declared_by` metadata.
Apply may create a missing record or update a record with the same owner. It
refuses implicit takeover and all takeover of a record owned by another
integration. Protocol 5 adds an exact `adopt_unowned` allowlist for an unowned
record. The target and declaration must name the same explicit Stable ID, and
the definition and relevant relationships must already match. Plan reports an
invalid takeover as a structured conflict, while apply refuses it before any
write. Omitted declarations are marked
retired and disappear from active graph and assurance checks without losing
their Stable IDs or history. Reintroducing one clears retirement on the same
record. Fields outside the small TypeScript interface are preserved;
source references declared by the spec are added rather than replacing
external references.

Plan distinguishes created, updated, moved, retired, conflicted, and unchanged
resources. A valid adoption uses the moved state because it assigns the first
Declaration address, and its changes contain only owner and address. An
identity-preserving Rule move updates the active owned
Requirement relationships. Historical relationships attached to retired
records remain canonical history. The POC still has no hard deletion,
ownership-transfer, or automatic ambiguous rename operation.

Implementation relationships follow the same non-destructive rule. Removing
`implementedBy` retires the binding owned by that spec and makes the active Rule
unimplemented. Plan reports an `implementation` field change on the Rule;
reintroducing the relationship reuses its binding ID, and changing the exported
target updates that binding in place. Other specs' bindings remain untouched.

## Verification relationships

`verify` reports the owner, file, and key it just ran and nothing wider.
Pointing one of those keys at a different Rule from the same file retires the
binding it replaced; calling it again reactivates the same binding ID. Retired
bindings stay in canonical exports as history and stop making the Rule appear
verified. Bindings whose test file disappeared are surfaced by the stale gate,
because one run cannot vouch for a file it never executed.

## Verification evidence

Callback results are volatile run evidence, so they live under:

```text
.provenance/cache/scopes/<scope>/verification-runs.jsonl
```

Each `verify` call names a stable owner-local binding key. Rust materializes one
canonical Verification binding for that key and Rule, while method, repository
path, and symbol remain updateable facts. Each run cites that binding and
carries the canonical Rule ID, execution context, status, timestamps, and an
optional serialized error. A typed handle starts the run with its declaration address; Rust resolves
that address to the canonical ID and rejects an unapplied declaration before
Node executes the callback. Keeping runs in the existing derived cache prevents
every local test from dirtying Git-tracked canonical state. Declarations
themselves remain in `.provenance/state` and therefore appear in exports,
traceability queries, checks, graph references, and generated wiki pages like
records created by the existing CLI.

The wiki and validating coverage scan consume canonical typed bindings alongside
scanner-discovered bindings. Runtime results remain separate and are queried
with `sdk verification-runs`; durable relationships are queried with
`sdk verification-bindings`. Stale analysis treats a changed typed verification
path as disturbed evidence without executing the callback.

## Compile-time result

The useful guarantee is ordinary TypeScript referential integrity. The valid
fixture follows `shareLinks.requirements.sharing.rules.expiry` and typechecks. A
second fixture renames that Rule key but leaves the nested access unchanged;
`tsc` fails with TS2339. This proves only that the verification code refers to a
declared Rule handle. It does not prove that the callback tests the right
production behaviour.

## Coexistence with existing bindings

The `@provenance/rules` identity helpers, scanner patterns, Rust attributes,
and comment directives are unchanged. Typed declarations create the same
canonical Rule records those bindings cite. A codebase may therefore keep a
scanner-recognized implementation binding while tests use imported handles.

The experiment resolved the semantic question it exposed. A Rule is an
independent behavioural obligation, so a typed declaration may materialize a
valid Rule before any production function realizes it. `#[rule]` and the
equivalent language helpers bind a primary implementation; they do not define
the Rule. A missing implementation is reported as Unimplemented. Existing
Rule records, source-citation fields, decorators, attributes, and scanner
patterns keep their shape, so retrofit and typed authoring target the same
canonical model without a data migration.

## Answers from the POC

1. `shareLinks.requirements.sharing.rules.expiry.verify("local-key", callback)`
   is more natural than a repeated Rule ID marker for tests. The string
   identifies the test relationship, not the Rule. The built spec exposes an
   immutable typed Rule handle at that semantic path.
   Imports, rename, autocomplete, navigation, and find-references all work in
   the TypeScript toolchain. The explicit `defineSpec` / `apply(spec)` split is
   also easier to reason about than persistence triggered by module import or
   the first test. It still adds an apply step and a Rust binary prerequisite.
2. The façade remains small. It builds a desired-state document, freezes typed
   handles, invokes four commands, wraps callbacks, and serializes errors.
   Reconciliation, canonical IDs, source-kind mapping, ownership checks, graph
   writes, and evidence validation remain in Rust.
3. One-shot child processes are sufficient. Platform packages now supply the
   matching engine without a global CLI, install script download, Rust toolchain,
   daemon, or gRPC.
4. Typed declarations coexist with external state by refusing implicit and
   foreign-owned takeover. Exact per-target adoption is the only unowned
   transition. Retirement applies only to records owned by the same spec, and
   history and fields outside the façade stay canonical.
5. The operations are portable: declare a source, requirement, or rule; apply
   desired state; begin a verification; run a language callback; complete the
   verification. No operation depends on a TypeScript-only runtime concept.

The next decision is how durable verification relationships and runtime
evidence should shape CI policy. Typed bindings already join coverage, stale
detection, wiki views, and semantic change plans. More languages are not the
next task.
