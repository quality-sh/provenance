# Programmable graph change proposals

Status: exploration for human review  
Repository evidence: `ef84911d7e8fe8d714899508f53c707b090de2cc`  
Tracking epic: `provenance-46p`  
Documentation task: `provenance-r60`  
SDK redesign task: `provenance-46p.1`

## Question

Should an SDK declare the desired Provenance graph as static source code, or should it let a
program construct a reviewable graph change and submit that change to the Provenance engine?

This document records the second direction for further design. It does not approve an interface,
an architecture decision, or a storage migration.

## Direction under study

The SDK is a programmable client for Provenance graph transactions. A program can query the graph,
run host-language logic, and construct typed graph operations. The engine then normalizes those
operations, plans their semantic effects, and returns a digest. An actor can approve that exact
plan. The engine commits it only if the graph and plan still match.

The program is not the desired state. It can be temporary and can be deleted after it produces the
change. The engine does not store or rerun the program.

This direction changes the role of the existing requirements-as-code work:

- `plan` and `apply` are prior art for the transaction module.
- Typed builders can become adapters that construct graph changes.
- Checked-in declarations can remain an optional adapter for code-owned records.
- Static declarations do not become the universal authoring model.
- A new declarative language and structural editing of SDK source are out of scope.

## Why this is not infrastructure as code

Infrastructure as code normally treats a checked-in document as desired state and repeatedly
reconciles the world to that document. That shape gives Git review, convergence, and reproducible
automation. It also makes the document a continuing authority.

A graph change program has a different lifetime:

1. It reads a graph revision.
2. It constructs a bounded change.
3. The engine plans that change.
4. An actor reviews the semantic plan.
5. The engine commits the exact approved change or refuses it.
6. The program can disappear.

The canonical graph holds the result. A transaction audit can hold the normalized input, actor,
approval, digest, and result. The source program has no later authority.

This model is close to a migration builder or a unit of work. It is not a permanent graph
specification.

## Draft domain language

These terms are candidates. They do not yet belong in `CONTEXT.md`.

### Change program

Host-language code that queries Provenance and constructs a Change Set. A Change Program can use
functions, conditions, imports, and local data. Provenance does not inspect, preserve, or execute
its source.

### Change draft

A client-side builder that holds incomplete operations and proposal-local references. It is not a
wire document and has no canonical status.

### Change Set

A closed, language-neutral document of typed graph operations. It contains no callback, source-code
expression, or executable host-language value. It is the input to planning.

### Planned Change

The deterministic result of evaluating one Change Set against one graph revision. It contains the
normalized operations, resolved record identities, semantic effects, warnings or refusals, base
revision, and digest.

### Approval

An actor's acceptance of one Planned Change digest. Approval of a digest is not approval of the
Change Program that produced it.

### Committed Transaction

The atomic graph publication that results when the engine recomputes and commits an approved
Planned Change. It records the new graph revision and the exact applied digest.

### Working state

Persisted shaping or coordination data that lets work continue across sessions. Fog, claims, and
open shaping state are working state. They are not ratified graph intent.

### Naming conflict: Proposal

`CONTEXT.md` already defines **Proposal** as an immutable candidate definition in the ideation
lifecycle. Assertions and Dispositions derive its effective state. The new term must not silently
replace that meaning.

The SDK redesign must choose one of these positions:

1. Use **Planned Change** or **Change Proposal** for the transaction concept and keep Proposal as it
   is.
2. Define an explicit relationship between an ideation Proposal and a Planned Change.
3. Unify the concepts through a separate domain decision and migration.

This exploration uses **Planned Change** for the transaction result. It uses **change proposal**
only as an informal description of the direction.

## The deep module and its seam

The proposed deep module is the graph transaction module. Its external interface should remain
small. The SDK syntax is an adapter at this seam, not the seam itself.

An illustrative interface has two write operations:

```text
plan(ChangeSet, BaseRevision, Principal) -> PlannedChange | Refusal
commit(ChangeSet, BaseRevision, ExpectedDigest, Approvals) -> CommitResult | Refusal
```

Names and fields are illustrative. The next design pass must design the interface more than once.

The module hides these behaviours:

- identity allocation and identity continuation;
- ownership and adoption checks;
- reference and relationship validation;
- ASD-STE100 write gates;
- affected-Rule and review analysis;
- deterministic ordering and digest construction;
- optimistic concurrency checks;
- approval policy;
- repository locking;
- staged publication and crash recovery;
- graph revision advancement;
- projection invalidation.

The module earns depth if every write adapter gets these behaviours without reimplementing them.
Likely adapters include the TypeScript SDK, other language SDKs, CLI commands, agents, a web UI,
full-scope import, and optional checked-in declarations.

Structured graph queries can remain a separate read interface. The current engine protocol already
has eight named query operations. A REPL can compose the read interface and transaction interface
without adding a new command or query language.

## Required invariants

Any credible interface must preserve these invariants:

1. Planning does not write canonical graph state.
2. Equal Change Sets against equal base state produce equal semantic plans and digests.
3. Approval binds to normalized operations and their semantic effects, not source code.
4. Commit recomputes the plan under the publication lock.
5. Commit refuses a stale base revision.
6. Commit refuses when the recomputed digest differs from the approved digest.
7. A multi-record change commits as one publication or not at all.
8. Ownership checks happen before the first write.
9. Unknown operation kinds and unknown fields fail closed.
10. A successful commit advances the graph revision exactly once.
11. Query projections expose the graph revision that they represent.
12. A stale projection never returns results without a visible stale condition.
13. The engine never executes host-language proposal code.

## Conceptual flow

```text
SDK, agent, CLI, or UI
          |
          | constructs
          v
      Change Set
          |
          | plan at graph revision N
          v
    Planned Change
    semantic effects
    resolved identities
    digest D
          |
          | actor approves D
          v
       Approval
          |
          | commit under lock
          | require revision N and digest D
          v
Committed Transaction ----> graph revision N+1
          |
          +---------------> projection revision N+1
```

## SDK code is an untrusted adapter

A Change Program can run arbitrary host-language code. That is useful for ergonomics and dangerous
as an approval target. The engine must treat the program as an untrusted client.

The safe boundary is the normalized Change Set:

```text
arbitrary host code -> closed typed operations -> engine validation
```

The engine can accept values that cross this boundary. It cannot accept callbacks, syntax trees,
closures, module paths whose evaluation supplies graph meaning, or claims about effects that the
engine does not recompute.

Commit must not rerun the Change Program. It receives the original Change Set, base revision,
expected digest, and approval references. It recomputes only engine-owned semantics.

## Identity without immutable source keys

Static declarations currently need owner-local declaration addresses. Renaming a key or moving a
declaration can change that address. The typed-spec implementation contains limited identity
continuation and explicit-id rules.

A temporary Change Program needs a different model. A Change Draft can use proposal-local handles:

```ts
// Illustrative only. This is not a proposed SDK interface.
const requirement = change.requirement.create({
  ref: "expiry_requirement",
  statement: "Shared links expire within seven days.",
})

change.rule.create({
  ref: "expiry_rule",
  requirement,
  statement: "The system rejects a shared-link lifetime longer than seven days.",
})
```

Planning resolves each local handle to a proposed canonical Stable ID. The mapping is part of the
Planned Change and its digest. Commit uses that exact mapping.

After commit:

- a rename changes a mutable name or statement, not the Stable ID;
- a move changes relationships, not the Stable ID;
- a retry uses the same Change Set identity and local handles;
- a changed Change Set requires a new plan and approval;
- references between records created together use local handles before they use canonical IDs.

The SDK redesign must decide who supplies new Stable IDs. Credible choices include client-supplied
opaque IDs, engine-proposed IDs derived from a Change Set identity and local handle, or provisional
IDs returned by planning. The decision must preserve deterministic planning and safe retry.

## State classes

The current JSONL store contains records with different meanings. Persistence alone does not make a
record ratified product truth.

| State class | Examples | Expected write policy |
|---|---|---|
| Graph intent | Sources, Requirements, Rules, Resolutions, Boundaries, Domains, authored edges | Planned Change; approval policy can depend on scope and risk |
| Working state | Fog, claims, open Questions and Topics, shaping threads | Revision-checked transaction; usually no approval ceremony |
| Immutable audit | current Proposals, Assertions, Dispositions, landings | Lifecycle-specific append operation; never rewritten |
| Engine-derived durable state | retirement markers, evidence review records | Engine writes as a consequence of a transaction |
| Volatile evidence | verification runs | Cache; never canonical graph state |
| Pure projection | SQLite, wiki, coverage and frontier reports | Rebuildable and revision-stamped |
| Code-owned state | scanned bindings and declarations owned by an integration | Owner reconciliation through the same transaction kernel |

The transaction mechanism can be universal without making every transaction an approval-bearing
Planned Change. Approval is policy over a transaction, not the transaction mechanism itself.

## Fog as working state

The current `Requirement` record has an optional `fog` string. `requirements fog set` and
`requirements fog clear` rewrite that Requirement in the JSONL store. Materialization copies the
value into `requirements.fog` in SQLite, and the wiki can render it.

The shaping model defines fog as deliberately unstructured text for decisions and investigations
that cannot yet be stated as Questions. It prevents premature graph records. It must persist across
agent sessions, but it is not settled desired state.

A future interface can expose a low-ceremony, revision-checked working-state operation:

```ts
// Illustrative only.
await workspace.setFog({
  requirement: "req_share_links",
  text: "Access auditing and revocation behaviour remain unclear.",
  expectedRevision,
})
```

When fog becomes precise, one transaction can create the resulting Questions and clear or reduce
the fog. Atomic graduation prevents the same concern from remaining in both forms after a partial
write.

The SDK redesign must not force a PM to approve every fog edit. It must also not let a stale agent
silently overwrite newer working state.

## What JSONL does in this model

Today, `.provenance/state/` JSONL is the canonical persisted store. It contains more than the typed
SDK declaration document can express. It also has deterministic ordering, stable record IDs,
sharding, repository locks, and a record-keyed Git merge driver.

The proposal direction does not require an immediate storage replacement. JSONL can remain the
engine's private current-state serialization while humans and agents stop authoring it directly.
All programmatic writers pass through the transaction module.

This separation lets the project decide the storage question later:

- Keep Git-backed JSONL for offline clones, readable diffs, merge, and revert.
- Replace it with another canonical store and export review artifacts to Git.
- Keep JSONL as a publication format while another store serves live transactions.

No SDK should depend on that choice. The engine protocol is the seam.

The operation surface must eventually cover all user-authored graph families and relationships.
Otherwise raw JSONL or a second mutation interface remains necessary. Engine-derived and volatile
state does not need a public create/update operation merely because JSONL can represent it.

## SQLite projection freshness

Today, `.provenance/cache/provenance.db` is a rebuildable projection of canonical JSONL.
Materialization is a separate operation. The SDK query operations read the state store directly,
not SQLite. There is no shared graph revision that proves a projection is current.

The transaction model makes freshness explicit:

```text
canonical graph revision: 185
SQLite projection revision: 184
result: projection is stale
```

Each successful commit advances a graph revision. Each projection stores the revision that it
represents. A read adapter compares them before it serves results.

If the projection is behind, the adapter can catch up, rebuild, use a canonical read path, or return
a typed stale-projection refusal. It must not silently serve old data. A file watcher can prewarm the
projection, but a watcher is not the correctness mechanism.

A failed projection refresh does not roll back a committed graph transaction. The stored projection
revision remains old, so the next read detects the condition.

## Product-manager scenario

A product manager asks an agent to change the shared-link expiry rule from 30 days to 7 days. The
manager has no local checkout and no GitHub access.

1. The agent queries the relevant Requirement, Rule, Resolution, evidence, and graph revision.
2. The agent constructs a Change Set that restates the Requirement, records the Resolution, updates
   the Rule, and changes any required relationships.
3. The engine plans the Change Set and reports affected evidence, review consequences, resolved
   identities, refusals, and digest.
4. The agent renders a short semantic summary for the manager.
5. The manager approves that digest through an allowed approval carrier.
6. A trusted adapter submits the Change Set, base revision, digest, principal, and approval.
7. The engine recomputes the plan under the lock and commits it or returns a typed stale refusal.
8. The manager receives the new graph revision and committed semantic result.

No step generates, edits, or reverse-engineers SDK source. Git can still receive the resulting
canonical-state commit when repository policy requires it.

## Developer and Git scenario

A developer changes code and wants to add a Rule, implementation binding, and verification binding.

1. A checked-in declaration adapter or a temporary Change Program constructs the typed operations.
2. The scanner supplies code-owned evidence sites and anchors.
3. The engine plans the combined semantic change against a base revision.
4. The developer reviews the plan and the repository diff.
5. CI commits or verifies the exact digest on a branch.
6. Merge and later Git revert remain available.

The transaction module supplies the same ownership, identity, validation, and publication behaviour
as the PM path. The adapters and approval policies differ.

## Disposition of the existing IaC work

Do not delete the existing implementation before the transaction seam exists. Preserve and reuse:

- plan versus apply;
- typed, language-neutral engine documents;
- reconciliation and deterministic ordering;
- ownership, adoption, and retirement;
- identity resolution;
- statement diagnostics;
- affected-evidence reporting;
- publication locking and recovery;
- versioned SDK protocol and structured query operations.

Pause or reject these directions unless later evidence reopens them:

- expand static SDK declarations to cover the whole graph;
- make checked-in declarations the sole source of graph truth;
- create a new declarative language;
- structurally edit arbitrary TypeScript to apply graph changes;
- infer canonical identity from temporary source layout;
- require GitHub access for every approval.

After the transaction core exists, measure whether checked-in declaration builders still provide
enough value for code-owned records to justify their interface and maintenance cost.

## Failure cases the design must make explicit

The redesign must specify typed refusals for at least these cases:

- stale base revision;
- approved digest differs from the recomputed digest;
- approval is absent, revoked, malformed, or for another change;
- principal lacks authority for an operation;
- operation targets another scope;
- operation targets a record owned by another integration;
- explicit adoption does not match the canonical record exactly;
- unknown operation kind or field;
- invalid reference or relationship;
- ambiguous identity continuation;
- duplicate create caused by retry or replay;
- ASD-STE100 write-gate failure where the gate applies;
- multi-record validation failure;
- projection revision is behind canonical state;
- transaction exceeds defined size or complexity limits.

## Non-goals

- Do not create a custom command language, query language, DSL, or textual grammar.
- Do not make the engine execute host-language code.
- Do not require a hosted service or daemon for correctness.
- Do not replace Git before the transaction model proves that it preserves the needed workflows.
- Do not turn every working-state edit into a high-ceremony approval.
- Do not treat SQLite or another rebuildable projection as canonical without a separate decision.
- Do not introduce a canonical global event log as part of this exploration.
- Do not claim cryptographic human identity from the current actor allowlist.
- Do not settle the existing Proposal naming conflict by implication.

## Open human decisions

1. Does the product use `Planned Change`, `Change Proposal`, or another term for the new primitive?
2. What relationship, if any, exists between a Planned Change and the current immutable Proposal
   lifecycle?
3. Which operations enter the first Change Set version?
4. Does every transaction receive a durable audit record, or do Git and existing Dispositions carry
   part of the audit chain?
5. What is the graph revision: a monotonic serial, a content digest, or both?
6. Who supplies new Stable IDs, and how does retry preserve them?
7. Which scopes or operation kinds require approval?
8. Which carriers can record approval for a PM without GitHub access?
9. Does a successful local commit always create a Git commit, or can repository policy choose direct
   working-state publication?
10. Which working-state operations can commit immediately after revision and ownership checks?
11. What is the long-term role of checked-in SDK declarations for code-owned records?
12. Does JSONL remain canonical storage, become a publication format, or leave after the transaction
    interface is stable?

## Brief for the SDK redesign agent

Reimagine the SDKs around programmable graph changes, not requirements as code. Do not treat the
illustrative TypeScript in this document as an interface proposal.

Use the Design It Twice discipline. Produce at least three substantially different interfaces:

1. Minimize the interface and aim for one to three entry points.
2. Optimize for a direct agent or product-manager workflow.
3. Optimize for a developer who composes changes in a host language and uses Git review.
4. If useful, add a design that separates read and write adapters sharply.

For each design, provide:

- the full caller interface, including invariants, ordering, and errors;
- a PM-with-agent example;
- a temporary developer Change Program example;
- a fog update and fog-graduation example;
- a multi-record change with proposal-local references;
- plan, approval, stale-plan refusal, and commit behaviour;
- what the transaction module hides;
- how TypeScript, another language SDK, CLI, and web adapters share semantics;
- how the design prevents approval of arbitrary source code;
- how it avoids a second graph specification;
- trade-offs in depth, locality, testability, and schema evolution.

Compare the designs before recommending one. Keep product decisions and engine semantics separate
from TypeScript ergonomics.

## Smallest decisive prototype

A bounded prototype can test the transaction primitive without redesigning every SDK.

The artifact accepts one closed Change Set that can create and update Requirements, Rules,
Resolutions, and their relationships. It exposes plan and commit through the existing engine
protocol. Planning returns a base revision, normalized operations, proposed IDs, semantic effects,
and digest. Commit requires the original Change Set, base revision, expected digest, and optional
approval references.

The prototype is decisive only if it proves:

- equal input and base state produce an equal plan and digest;
- a changed base causes a typed stale refusal;
- a changed Change Set causes a digest refusal;
- records created together can refer to proposal-local handles;
- a multi-record change publishes atomically;
- ownership conflicts refuse before writes;
- the committed result matches the approved semantic plan;
- a low-ceremony fog edit still uses revision checks;
- the SQLite projection exposes its revision and cannot silently serve a stale result;
- no host-language code crosses the engine protocol.

## Repository evidence

- `CONTEXT.md` defines the existing Proposal, Engine protocol, Declaration owner, Declaration
  address, and Retired declaration terms.
- `docs/state-format.md` defines JSONL as the canonical persisted store and documents deterministic
  ordering, locks, and merge behaviour.
- `docs/cache.md` defines SQLite as a rebuildable projection that is never the source of truth.
- `docs/shaping.md` defines fog as deliberately unstructured working material that can graduate into
  Questions.
- `crates/provenance-core/src/model/artifacts.rs` stores fog as an optional Requirement field.
- `crates/provenance-store/src/state_store/writers.rs` implements fog set and clear as Requirement
  mutations.
- `crates/provenance-store/src/cache/materialize/graph_records.rs` projects Requirement fog into
  SQLite.
- `crates/provenance-core/src/protocol/typed_spec.rs` defines the current closed typed declaration
  document.
- `crates/provenance-core/src/authoring/` builds language-neutral typed documents and handles.
- `crates/provenance-store/src/state_store/typed_specs.rs` shares plan and apply reconciliation.
- `crates/provenance-store/src/state_store/typed_specs/` contains identity, ownership, adoption,
  lifecycle, and relationship behaviour that a transaction module can reuse.
- `crates/provenance-store/src/operations/plan.rs` constructs the current semantic typed-spec plan
  and affected-Rule evidence.
- `crates/provenance-store/src/publication.rs` owns repository publication locking and recovery.
- `crates/provenance-store/src/operations/queries.rs` exposes the current structured graph read
  operations.
- `crates/provenance-store/src/graph_reference/export.rs` supplies a canonical graph digest
  precedent.
- `crates/provenance-cli/src/cli/sdk.rs` and `packages/provenance/src/protocol.ts` expose the current
  versioned engine protocol to SDK callers.
- `docs/adr/0004-typed-declarations-retire-in-place.md` and
  `docs/adr/0005-typed-implementation-bindings-retire-in-place.md` record durable retirement instead
  of omission-driven deletion.
- `docs/adr/0007-requirement-changes-put-evidence-up-for-review.md` records the evidence-review
  consequence of Requirement restatement.
