# SDK as authoring surface and agent API — CodeMode-informed research

Status: exploration for human review
Repository evidence: `2e88c9b8173cb8ed4400fc2be5c192da002463cf`
Tracking epic: `provenance-46p`
Companion direction document: `docs/research/2026-08-27-programmable-graph-change-proposals.md`

## Question

Can the Provenance SDKs serve two roles at once:

1. An authoring surface that programs use to construct reviewable Proposals and graph changes.
2. A surface that AI agents address by generating scripts against a versioned API, to run
   complex queries over the application.

OpenCode's CodeMode (`@opencode-ai/codemode`) is the stated inspiration for role 2: give an
agent a schema-described tool object tree and a confined JavaScript orchestration language,
instead of one tool call per question.

This document records the current state of the app, the CodeMode pattern from its primary
source, how existing mechanisms map onto both roles, and the constraints any pivot inherits.
It approves nothing.

## Summary

Provenance already has most of the hard parts for both roles; they are not joined yet.

For authoring: the ideation store already accepts a closed, language-neutral batch document
(`IdeationLandingBatch`) through a validate-before-write transaction
(`write_ideation_batch`). One aggregate validator guards every entry path (direct writes,
swarm landing, import, check, materialization). The swarm-backtrace command proves a machine
batch can enter today without touching shards directly. But none of this is reachable from
the `sdk` protocol or the TypeScript package: the protocol contains no proposal types. Today
an agent authors proposals only by invoking CLI verbs with inline JSON.

For agent queries: the engine already exposes eight named structured query operations, each
one JSON request in and one bounded JSON answer out, with an envelope
(`protocol_version`, `operation`), paging bounds, and fail-closed input decoding. The
TypeScript functions over them add no logic of their own. This shape converts mechanically
into a CodeMode-style tool object tree. What does not exist anywhere in the repo is the host
for such a runtime: there is no daemon, socket, MCP server, or long-running process for graph
work, and the engine deliberately refuses requests whose semantics it does not recompute.

CodeMode supplies a proven pattern for the missing piece on the client side: a restricted
JavaScript interpreter over supplied tools only, failures as typed data diagnostics,
budgeted discovery, host-owned authority ("a program cannot gain authority through prose or
generated code"), and approval modelled above the runtime, never inside it. Every one of
those boundaries has a direct analogue in Provenance's current rules, except two gaps:
refusals cross the wire as strings rather than structured values, and proposal writing has no
programmatic seam at all.

## Detailed findings

### 1. Current programmatic surface

#### 1.1 TypeScript package (`packages/provenance`, `@quality-sh/provenance` 0.2.2)

Configuration and environment (`src/index.ts`):

- `configure(options)` resets the global declaration registry (`index.ts:193-196`).
- Defaults read `PROVENANCE_BIN`, `PROVENANCE_REPO`, `PROVENANCE_SCOPE`,
  `PROVENANCE_SPEC_OWNER`, `PROVENANCE_VERIFICATION_OWNER` (`index.ts:449-457`).

Write operations:

- Three authoring styles over the same wire document: builder mode
  (`defineSpec(key, build)`, `src/spec.ts:211-227`), fluent module-level factories
  (`source()`/`requirement()`/`rule()`, `src/index.ts:198-242`), and bound/typed handles
  (`defineSpec(key)` without builder, `src/bound-spec.ts`).
- `apply(spec?)` reconciles the full desired-state document (`index.ts:257-271`).
- `plan(spec)` previews the same reconciliation without writing (`index.ts:273-281`) and adds
  `affected_rules` to the result (`src/protocol.ts:162-164`).
- `RuleHandle.verify(key, callback)` auto-applies a dirty registry, starts an engine-side run,
  executes the callback in Node between begin and complete, and serializes errors
  (`index.ts:357-430`).

Structured queries — each is a thin forward with no local traversal or filtering
(`index.ts:286-318`; contract documented in `docs/typescript-sdk-poc.md:54-56`):
`get`, `search`, `neighbors`, `trace`, `impact`, `evidence`, `stale`, `resolveSymbol`.

Transport (`src/engine.ts`):

- One subprocess per call: `spawn(engine, ["sdk", command, ...])`, JSON in on stdin, one JSON
  document out on stdout (`engine.ts:61-74`). No daemon, socket, FFI, or native addon
  (`docs/typescript-sdk-poc.md:58-62`).
- First call per binary runs an `"info"` handshake cached as a promise; the SDK refuses any
  engine whose `protocol_version !== 5` (`engine.ts:17,31-45`).
- Every call passes `--repo`, `--scope`, `--format json` (`engine.ts:53-60`).

Error crossings (`engine.ts:75-96`):

- Spawn failure → generic start failure error naming `PROVENANCE_BIN`.
- Non-zero exit → `Error("...command '<cmd>' failed (<code>): <stderr/stdout>")`. Engine
  refusals therefore arrive as **strings**, not typed values.
- Unparseable output → error wrapping the parse failure as `cause`.

Result and envelope types live in `src/protocol.ts`: `TypedSpecDocument` (`:59-67`),
`ReconcileState` created|updated|moved|retired|conflict|unchanged (`:69-75`),
`ApplyResult` (`:93-104`), `QueryEnvelope {protocol_version, operation}` (`:206-209`), paged
answers carry `{limit, has_more}` (`:220-223`), and the diff vocabulary for stale evidence
(`:310-342`). `NodeType`/`EdgeType` mirror the Rust enums exactly (`:176-193`).

Platform packaging ships the Rust binary per triple via optional dependencies
(`src/engine-packages.ts:3-8`; four targets linux-x64-gnu, win32-x64-msvc, darwin-x64,
darwin-arm64). Install performs no binary download (`scripts/package-engine.js:6,46-49`).

#### 1.2 Engine-side CLI operations (`crates/provenance-cli/src/cli/sdk.rs:16-128`)

16 subcommands today:

| Group | Operations |
|---|---|
| Write | `plan`, `apply`, `begin-verification`, `complete-verification` |
| Read (graph) | `get`, `search`, `neighbors`, `trace`, `impact`, `evidence`, `stale`, `resolve-symbol` |
| Read (evidence) | `verification-runs --rule <id>`, `verification-bindings --rule <id>` |
| Utility | `info` (version negotiation), `check-statement` (stateless ASD-STE100 check) |

Contract details:

- Empty stdin refused; every input struct uses `#[serde(deny_unknown_fields)]`
  (`handlers/sdk.rs:105-113`), as do all query inputs (`crates/provenance-core/src/protocol/query.rs:41-161`)
  and verification inputs (`state_store/inputs.rs:213-257`).
- Guards: `ensure_protocol_version` refuses anything other than 5
  (`protocol/protocol.rs:48-63`); queries bounded to limit default 50 / max 200 and trace depth
  default 3 / max 10 (`protocol.rs:28,34-37,66-81`); `take_page` truncates with `has_more`
  (`protocol.rs:84-88`).
- Responses carry `QueryResponse<Result> {protocol_version, operation, ...}` 
  (`protocol/response.rs:16-33`).

Reads used by CI and agents are write-safe by construction: `coverage scan` derives absence
findings at scan time and stores nothing, and `sdk stale` "performs no review-trigger firing,
agent work, requirement extraction, or state write" (`docs/cli.md:188-209`).

#### 1.3 Reconciliation internals a change module could reuse

These exist today and behave like a transaction kernel, scoped to the three typed families
(sources, requirements, rules):

- Pure authoring kernel, no IO (`core/src/authoring/authoring.rs:1-7`); address grammar with
  four legal shapes (`authoring/addresses.rs:10-31`); first-error-stable validation order pinned
  by rule annotation (`store/src/state_store/typed_specs.rs:66-103`).
- Identity resolution ladder owned by the store: existing id at address > explicit id >
  well-formed canonical key > slug plus SHA-256 of `owner\0address`
  (`store/src/state_store/typed_specs/identity.rs:137-177`).
- One pipeline for plan and apply (`ReconcileMode::Plan | Apply`): validate, resolve ids,
  ownership decision via `adoption::decide` (same-owner update, foreign-owner conflict,
  unowned adoptable only by exact allowlist target with identical definition),
  reconcile, then apply-only gates in fixed order: STE100 statement gate before any write,
  shard replacement, edge reconciliation, review raising
  (`typed_specs.rs:57-231`; adoption in `typed_specs/adoption.rs:311-329`).
- Managed edges limited to `references` (Source→Requirement) and `produces`
  (Requirement→Rule); deletion filtered to endpoints this spec owns
  (`typed_specs/relationships.rs:141-163`).
- Affected-Rule set computed at plan time (changed rules ∪ new implementation bindings ∪ rules
  reached through `produces` ∪ nested rules) (`operations/plan.rs:101-152`); restatement reviews
  previewed by plan, written by apply, cleared by a later verification run beginning
  (`operations/plan/evidence.rs:63-107`; `typed_specs.rs:238-273`; `verification_runs.rs:112`;
  ADR 0007).
- Publication safety: advisory repository lock with reentrancy guard, symlink-refusing real
  directory check, pending-publication marker with Prepared→BackupCreated→Published recovery
  (`store/src/publication.rs:25-208`).

#### 1.4 Absent surfaces

No MCP server or client, JSON-RPC, LSP, Unix socket, TCP API, daemon, or file watcher exists
anywhere in the workspace. The only listeners are two static-site servers (`docs serve`,
`wiki serve`) with GET-only routing (`cli/docs/web.rs:174-198`; `wiki/site.rs:129`). The
in-process Rust SDK (`crates/provenance-sdk`) exposes the same operation set through library
calls instead of subprocesses (`settings.rs:1-33`).

### 2. Current Proposal lifecycle machinery

Domain terms (`CONTEXT.md:119-145`): a Proposal is an immutable candidate definition always
authored as `proposed`; Assertion is immutable unblocked-adjudication evidence; Disposition is
the sole immutable authority for accepted/rejected/deferred; ratification-through-action means
one accepted disposition names a canonical artifact a human action already produced.

Data model (`crates/provenance-core/src/model/ideation*.rs`):

- `ProposalCard` stores `promotion_state` literally (`model/ideation/proposals.rs:23-57`);
  the author gate rejects any modern row not stored as `proposed`
  (`model/ideation/lifecycle.rs:96-115`, annotated `rule_proposal_authored_as_proposed`).
- Effective state derived at read time: stored terminal wins (frozen legacy rows only), else
  disposition outcome, else assertion ⇒ asserted, else proposed
  (`lifecycle.rs:127-162`; exhaustive property tests `:232-339`).
- Seven-state `PromotionState` includes legacy-only duplicate/superseded
  (`model/ideation.rs:218-251`). `IdentityType` already distinguishes human, agent, service
  (`ideation.rs:74-93`).
- Dispositions may name a `CanonicalArtifact` (source, requirement, resolution, rule) and an
  optional external-action tuple `(system, scope, kind, key)`
  (`model/ideation/dispositions.rs:17-49`). Acceptance requires a prior assertion unless a
  human actor names an existing same-scope same-kind artifact
  (`dispositions.rs:77-81`).

Validation choke point (`model/ideation/lifecycle/aggregate_validation.rs`): one `validate()`
covers schema versions, immutable-id uniqueness, actor allowlist, lineage acyclicity, must-
assert closure, disposition admissibility — used identically by direct writes, swarm landing,
import, `check`, and materialization (`:51-115`; ADR 0001:9-12). Actor IDs must appear in the
manifest allowlist set by `init --disposition-actor-id`; empty lists refuse with setup hint
(`aggregate_validation.rs:159-207`). The documents state plainly this is audit attestation
under repository access, not authentication (`docs/cli.md:396-399`; ADR 0001:14-17).

Storage and locks (`crates/provenance-store/src/shards.rs`): seven ideation shards under
`.provenance/state/scopes/<scope>/ideation/` — contributions (`:97-102`), synthesis packets
(`:104-109`), proposal cards (`:111-116`), dispositions (`:118-123`), legacy promotion
decisions (`:125-133`), assertions (`:135-140`), landings (`:142-147`). Mandatory lock order:
repository publication → scope lifecycle → shard (`layout.rs:53-74`; `docs/state-format.md:99-105`).
Readers overlay landing batches by id and project derived promotion_state
(`state_store.rs:290-350`). SQLite materialization copies lifecycle tables including the
derived state (`cache/materialize/collaboration_records.rs:45-76`; migrations 012–014). Graph
references exclude lifecycle families entirely (`graph_reference/projection.rs:11-28`).

The key primitive for this research is `write_ideation_batch`
(`state_store/ideation_batches.rs:41-124`), which commits one
`IdeationLandingBatch` atomically:

```text
IdeationLandingBatch {
  contributions?, synthesis_packets?, proposals?, assertions?, dispositions?
}
```

It enforces scope equality across all records (`:361-384`), merges proposals/assertions/
dispositions strictly immutably while contributions and synthesis packets replace only when
uncited by assertions (`merge_immutable` `:410-430`; freeze `:273-289,297-314`), runs the
qualifying-must-assert closure (`:97-106`), validates the complete aggregate before mutation
(`:107-116`), resolves canonical-artifact existence for every disposition (`:117-120`), and
appends the raw batch to `landings.jsonl` as an audit record (`:121`).

CLI verbs that touch proposals (`crates/provenance-cli/src/cli/ideation.rs`):

| Verb | Notes |
|---|---|
| `proposals create` (`:160-198`) | Flags for type/target/evidence/builds-on; optional `--assertion-id --synthesis-packet-id` writes proposal+assertion as one atomic batch (`handlers/proposals.rs:18-82`; `proposal_writers.rs:13-50`) |
| `proposals assert` (`:199-220`) | Refuses disposed proposals (`proposal_writers.rs:120-133`); human-gate variant removes blocking `required_human_decisions` then writes packet+assertion in one batch (`assert_proposal_after_human_decision` `:57-115`) |
| `proposals list` | Shows derived states (`handlers/proposals.rs:111-120`) |
| `proposals surface` (`:229-247`) | Demand-driven consultation on exact changed paths or typed territory; read-time view writing nothing (`proposal_surfaces.rs:88-156`) |
| `dispositions create` (`:249-289`) | Requires `--actor-id/-type/--rationale/--decision`; optional canonical-artifact pair and external-action quadruple |
| `contributions create/list`, `synthesis-packets create/list` | `--replace` allowed pre-assertion |
| `swarm-backtrace land` (`:6-19`) | Machine batch entry point; refuses dispositions outright (`handlers/swarm_backtrace.rs:77-80`); preflight checks unique ids, scope match, must-assert closure, replaceability (`:253-307`) |
| `schema show <artifact>` | Publishes the JSON Schema per artifact kind (`cli.rs:235-238`; handlers/schema.rs) |
| `validate <artifact> --input <file>` | Full closed-record validation of one file (`handlers/validate.rs:24-105`) |
| `import` / `export` | Lifecycle families carried whole; import stages, checks, preserves immutables byte-identically, publishes via backup rename (`handlers/import.rs:61-286`) |

The gap: none of this appears in the SDK protocol. `crates/provenance-core/src/protocol/*`
contains no proposal types, and `operations.rs:26-139` exposes no ideation functions. The
current machine path into proposals is the CLI alone, taking inline JSON or `@file` payloads
(`docs/cli.md:376-389`).

### 3. The CodeMode pattern (primary source)

Source: `github.com/anomalyco/opencode`, `packages/codemode/README.md` (fetched 2026-08-27;
package is workspace-private; opencode enables Code Mode by default for MCP servers,
https://opencode.ai/v2/docs/mcp-servers).

What CodeMode is: an Effect-native, confined code executor. The host defines tools with
`Tool.make({description, input, output, run})` using Effect Schema (or render-only JSON
Schema for adapters), arranges them into an object tree, and hands an agent a JavaScript
orchestration language over exactly that tree:

```ts
const runtime = CodeMode.make({ tools: { orders: { lookup: lookupOrder } } })
const result = yield* runtime.execute(`
  const order = await tools.orders.lookup({ id: "order_42" })
  return { id: order.id, needsAttention: order.status !== "complete" }
`)
```

Observed mechanics relevant to Provenance:

1. **Confined language, not a sandbox escape hatch.** No eval, dynamic imports, modules,
   classes, generators, timers, host globals, prototype mutation, custom promise
   constructors, or `.then` chaining. Common array/string/Object/Math/JSON operations, Date,
   RegExp, Map/Set (iterator-returning methods return arrays), URL helpers, real
   `Promise.all/allSettled/race` with at most 8 concurrent calls. Result serialization forces
   the plain-data boundary everywhere.
2. **Failures are typed data.** Ten diagnostic kinds — `ParseError`, `UnsupportedSyntax`,
   `UnknownTool`, `InvalidToolInput`, `InvalidToolOutput`, `InvalidDataValue`,
   `ToolCallLimitExceeded`, `TimeoutExceeded`, `ToolFailure`, `ExecutionFailure`. Unknown host
   failures are sanitized; tools publish safe messages explicitly via `toolError()`. The
   result carries a call-order `toolCalls` audit list even on failure.
3. **Schema boundaries do the guarding.** A tool never runs unless its input decoded; a
   result stays invisible unless its output decoded across the plain-data boundary
   (the README's stated Laws).
4. **Discovery scales the catalog.** Instructions inline a budgeted catalog (default ~2000
   estimated tokens, round-robin across namespaces so everything gets some representation) plus
   a built-in `tools.$codemode.search` with deterministic weighted matching, pagination, and
   exact-path lookup returning generated TypeScript signatures with JSDoc field comments.
5. **Limits are host policy, not library policy.** Three knobs — `timeoutMs`,
   `maxToolCalls`, `maxOutputBytes` — all unset by default; oversized output truncates with
   markers instead of failing. Hooks (`onToolCallStart/End`) expose observation without
   leaking inputs to the model.
6. **Authority lives entirely host-side.** Authentication, authorization, credentials,
   persistence, idempotency, approval, durable side effects are the host's job. The stated
   law: "A program cannot gain authority through prose or generated code. It can only exercise
   authority already present in the supplied tools."
7. **Approval is above CodeMode.** Listed non-goals include permission prompts, durable
   pause/resume, exactly-once side effects, and application authorization policy. Hosts should
   "expose only the currently authorized tools."
8. **Specs convert to tools mechanically.** `OpenAPI.fromSpec` yields one tool per operation;
   dotted operationIds become namespaces (`v2.session.get`); unsupported parameter encodings
   land in `skipped` instead of producing broken tools; auth resolution stays host-side and
   never model-visible.

Sibling patterns observed elsewhere, briefly (from web research, community sources):

- Generated typed clients from one authoritative contract, with committed generated source and
  a CI regeneration-drift gate (opencode `httpapi-codegen`).
- MCP remains relevant as the transport underneath: opencode defaults MCP integration to
  codemode, grouping server tools under normalized namespaces (opencode docs,
  https://opencode.ai/v2/docs/mcp-servers).
- Headless CLIs that stream JSON events and exit on idle (opencode `run --format json`;
  Codex `exec --json`) coexist with a persistent server; neither replaces the other.

### 4. How the two roles map onto what exists

#### Role A — SDK as authoring surface for proposals

What is ready:

- A closed, language-neutral batch document already exists and is the sole sanctioned atomic
  entry: `IdeationLandingBatch`. Its reader side refuses unknown fields and nested schema
  drift (`readers.rs:19-25,130-141`) — the same fail-closed posture as `TypedSpecDocument`.
- `write_ideation_batch` already behaves like the prior exploration's planned commit
  primitive: full-aggregate validation before mutation, strict immutable merge, deterministic
  append, raw-batch audit trail. Plan-then-commit can be composed from existing halves —
  `provenance validate <artifact>` and `schema show` supply per-record preflight, and readers
  project effective state without writes.
- Precedent for machine authorship with human-gated authority: swarm landing enters batches
  but cannot produce dispositions (`swarm_backtrace.rs:77-80`); assertion qualification closes
  over the complete aggregate so a qualified proposal without an assertion rejects the whole
  batch (`:274-286`); disposition actors need the manifest allowlist. The state classes table
  in the companion research document already classifies proposals/assertions/dispositions as
  Immutable Audit with a "Lifecycle-specific append operation" expected write policy — which
  is precisely what landing batches implement.
- `IdentityType {human, agent, service}` already exists in the model
  (`model/ideation.rs:74-93`), matching CodeMode's premise that agents are callers, not a
  separate authority class.

What is missing:

- Protocol exposure: no proposal-shaped input types, no sdk verbs for
  contributions/synthesis/proposals/assertions/surface/list. Everything routes through the
  CLI today.
- Typed refusals: engine refusals reach SDK callers as exit-code strings
  (`engine.ts:83-88`), while the companion document's failure-case list anticipates typed
  refusal cases (stale base, digest mismatch, absent/malformed approval, ownership, unknown
  fields, size limits). CodeMode's diagnostics-as-data shows one worked-out shape for this
  gap.
- Digest/approval binding for non-lifecycle graph intent remains an open design item in the
  companion document (its open decisions 1–12); nothing here resolves them.

Term collision carried forward unchanged: CONTEXT.md's Proposal (immutable candidate
definition, ideation lifecycle) versus the companion document's candidate transaction terms
(Planned Change / Change Set). Any SDK surface spanning both roles must pick a position
explicitly (companion doc, "Naming conflict: Proposal").

#### Role B — agents generating scripts against the API

What is ready:

- The eight structured queries have exactly the tool shape CodeMode consumes: named
  operation, schema-described input, bounded validated output, envelope carrying
  `protocol_version`/`operation`, explicit paging (`limit`/`has_more`),
  `include_retired` visibility gate. The TS wrappers add zero logic, so a tools object tree
  would be a mechanical projection — mirroring how `OpenAPI.fromSpec` treats one operation as
  one tool with dotted namespaces (e.g. `provenance.graph.search(...)`,
  `provenance.evidence.rule(...)`).
- Sequencing value is concrete with today's vocabulary: fan out `impact()` per changed Rule
  with `Promise.all`, branch on `review_required` before proposing edits, join
  `evidence()` results with `stale()` sites, aggregate multi-hop answers into one returned
  value instead of N sequential tool round-trips.
- Catalog growth pressure is foreseeable: 16 CLI operations + an eventual change-program
  surface approaches the scale where CodeMode's budgeted instructions and `$codemode.search`
  pattern matter; Provenance already has the discovery substrate in miniature via `schema show`
  (per-kind schemas a script generator can consult).
- Agent-safe reads are established policy: `--format json` on agent-facing commands
  (`docs/cli.md:28`), scan/stale write-nothing guarantees (`docs/cli.md:209`), bounded limits
  enforced engine-side.

What is missing:

- A host process for a confined runtime. Provenance's deliberate architecture is daemonless
  ("nothing here is a query language and nothing needs a daemon", `docs/cli.md:76-77`), with
  callbacks executed Node-side between short-lived engine processes
  (`docs/typescript-sdk-poc.md:33-37`). A CodeMode-equivalent therefore maps most naturally
  onto the existing seam: a Node/TS-side confined runtime whose `tools` wrap the sdk
  operations — the same placement where `verify()` already runs caller code — leaving the
  Rust engine as pure request/refusal adjudicator.
- Structured outputs good enough to script against deeply: answers are bounded but refusals
  are not typed, and several ideation views (surface reasons, qualification facts) are not in
  the protocol at all.

#### Authority mapping (CodeMode boundary ↔ Provenance rules)

| CodeMode host-owned duty | Current Provenance counterpart |
|---|---|
| Authentication/authorization | Repository and CLI access; manifest disposition-actor allowlist; declared-by ownership and adoption checks |
| Approval and durable side effects | Immutable dispositions; ratification-through-action naming canonical artifacts; "the agent never proceeds past a decision the human hasn't ratified" (`docs/shaping.md:86-94`) |
| Persistence/idempotency | Validate-before-write batches; lock ordering; publication marker recovery; byte-identical immutable preservation on import |
| Tool selection/scope | State classes (companion doc): graph intent vs working state vs immutable audit vs projections decide which operations exist at all |
| Diagnostics/safe messages | Fail-closed serde decoding, refuse-string exits (gap), typed plan conflicts reported as data (`protocol.ts:69-75`) |

CodeMode's own non-goals align with two standing Provenance non-goals in the companion
document: don't let the engine execute host-language proposal code, and don't turn working-
state edits into high-ceremony approvals. Both systems push ceremony outward: CodeMode to the
host, Provenance to adapters and approval carriers.

## Constraints inherited from documented decisions

Condensed from `docs/state-format.md`, `docs/cache.md`, `docs/cli.md`, `docs/shaping.md`, and
ADR 0001–0008; each cited in the source survey:

1. Proposal definitions, assertions, dispositions are immutable; state is always derived at
   read time; divergent duplicate ids fail closed (ADR 0001:9-11).
2. One aggregate validator guards every lifecycle entry path (ADR 0001:9-12).
3. Swarm-batch qualification runs against the complete existing-plus-incoming aggregate; bad
   evidence rejects the whole batch (`docs/cli.md:386-389`).
4. Canonical-artifact references resolve exactly `(scope, type, id)` and fail closed at every
   persistence, validation, materialization, and checking seam (`docs/cli.md:413-426`).
5. Lock order publication → lifecycle → shard is mandatory; multi-shard writers hold the
   publication lock for the whole operation (`docs/state-format.md:99-105`).
6. Ownership: `declared_by` reconciles; implicit takeover refused; adoption requires exact
   pre-matching definition and explicit Stable ID; foreign takeover always refused
   (`docs/state-format.md:9-18`; ADR 0008).
7. Omission retires owned declarations in place; hard deletion and owner transfer are separate
   unsupported operations (`docs/state-format.md:20-26`; ADR 0004).
8. Requirement restatement raises per-Rule review records; a later run clears only reviews
   raised before it; re-applying never reopens (`docs/state-format.md:55-63`; ADR 0007).
9. The single Rust ASD-STE100 checker gates direct writes, plan/apply, staged import, merged
   JSONL; unchanged statements resent must not block unrelated updates; rejection precedes all
   mutation (`docs/research/2026-08-15-simple-technical-english-configuration.md:69-71,225-233,344`).
10. Protocol evolution precedent: capability additions bumped the SDK protocol (4→5) while the
    canonical schema stayed at v1 (`docs/adr/0008:39-41`) — adding proposal operations fits
    that precedent.
11. SQLite is a rebuildable projection, never truth; materialization revalidates with the same
    aggregate validator (`docs/cache.md:3-9`).
12. Actors are allowlisted attestations, not authenticated identities; provenance derives
    territory only from exact paths and typed targets, never inference
    (`docs/shaping.md:176-194`; `docs/cli.md:401-411`).
13. Graph-reference digests exclude lifecycle families by definition
    (`docs/state-format.md:115-120`; `graph_reference/projection.rs:11-28`).

## Code references

Current SDK surface:

- `packages/provenance/src/index.ts:193-457` — configure, factories, apply/plan, queries, verify, env defaults
- `packages/provenance/src/engine.ts:17-96` — handshake, spawn-per-call transport, error crossings
- `packages/provenance/src/protocol.ts:59-233` — wire document, results, envelopes
- `crates/provenance-cli/src/cli/sdk.rs:16-128` — all 16 sdk subcommands
- `crates/provenance-cli/src/handlers/sdk.rs:105-113` — stdin contract
- `crates/provenance-core/src/protocol/protocol.rs:25-88` — version guard, bounds, paging

Reconciliation machinery:

- `crates/provenance-store/src/state_store/typed_specs.rs:57-231` — plan/apply pipeline
- `crates/provenance-store/src/state_store/typed_specs/identity.rs:137-177` — identity ladder
- `crates/provenance-store/src/state_store/typed_specs/adoption.rs:311-329` — ownership decisions
- `crates/provenance-store/src/publication.rs:25-208` — locking and crash recovery

Proposal machinery:

- `crates/provenance-core/src/model/ideation/lifecycle.rs:96-162` — authored-as-proposed gate, derived state
- `crates/provenance-core/src/model/ideation/lifecycle/aggregate_validation.rs:51-207` — validator choke point and actor allowlist
- `crates/provenance-store/src/state_store/ideation_batches.rs:41-124` — atomic landing batch commit
- `crates/provenance-cli/src/handlers/swarm_backtrace.rs:66-167` — machine batch entry with preflight
- `crates/provenance-cli/src/handlers/proposals.rs:18-150` — create/assert/list/surface handlers
- `crates/provenance-store/src/state_store/proposal_surfaces.rs:88-156` — demand-driven surfacing

External primary sources:

- `packages/codemode/README.md`, github.com/anomalyco/opencode (dev branch) — CodeMode contract: confined subset, diagnostics-as-data, Laws, limits, discovery, OpenAPI bridge, authority boundary
- opencode.ai/v2/docs/mcp-servers — Code Mode default for MCP tools
- opencode.ai/v2/docs/api — OpenAPI-published surface context

## Historical context

- `docs/adr/0001-immutable-proposal-lifecycle.md` — why lifecycle records never rewrite and
  why the validator is singular; import recoverability posture.
- `docs/shaping.md:159-211` — the output contract all producers follow
  (contributions → synthesis → proposals), dispose-on-demand philosophy, run-status excluded
  from durable state because skills own execution.
- `docs/typescript-sdk-poc.md` — the POC's answered questions (façade smallness, one-shot
  child processes sufficing, no-daemon stance, adoption semantics) define the ergonomics
  envelope any new surface inherits.
- `docs/research/2026-08-27-programmable-graph-change-proposals.md` — the sibling direction
  for transactional graph change; this document extends its adapter inventory ("SDK, other
  language SDKs, CLI commands, agents, a web UI") with a concrete mechanism reference
  (CodeMode) and locates the ideation store inside its state-class table.
- `docs/research/2026-08-15-simple-technical-english-configuration.md` — statement-gate
  behaviour that would bind any new write path equally.
- `skills/provenance-swarm-backtrace/SKILL.md` — ground rule: candidates land
  `promotion_state=proposed`, never active; orchestrator-only landing.

## Related research

- `docs/research/2026-08-27-programmable-graph-change-proposals.md` — the Change Set /
  Planned Change direction this pivot composes with.
- `docs/research/2026-08-09-rules-as-code-fiat-handoff.md` — superseded chain; background for
  why declarations stopped being the universal authoring model.

## Open questions

1. Terminology: which position toward the Proposal naming conflict (companion decisions 1–2)
   applies once an SDK constructs both ideation batches and graph-intent changes?
2. Entry point for programmatic proposal authoring: extend the sdk protocol with ideation
   operations (reusing `write_ideation_batch` under `landings.jsonl` semantics), keep CLI-only
   with `@file` payloads, or introduce a batch-planning step mirroring `sdk plan`? The 4→5
   protocol bump precedent suggests feasibility, not a choice.
3. Should refusals become typed objects across the wire (diagnostics-as-data à la CodeMode)
   before either role lands, given the companion document's failure-case inventory expects
   them?
4. Where does a confined execution runtime sit if adopted: Node-side within the TS SDK (where
   `verify()` callbacks already run), inside a future host the way CodeMode embeds in
   opencode's MCP handling, or both — and what does daemonless principle forbid?
5. Does `$codemode.search`-style discovery have value at 16 operations, or only after the
   change-program surface grows the catalog?
6. Which approval carriers beyond the repository allowlist suit the PM-without-GitHub scenario
   (companion open decision 8), and does the external-action correlation tuple suffice as the
   audit link for scripted approvals?
7. Do working-state operations (fog set/clear, topic claim/release, question answer) get
   low-ceremony revision-checked sdk verbs in the first version, per the companion document's
   fog section?
