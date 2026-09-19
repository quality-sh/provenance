# API contract v2 — frozen

This document is the contract for the restructured Provenance API. It records the
settled decisions of 2026-09-13 and freezes the surface for Phase 2. Do not extend
it without a recorded decision and an authorized compatibility change
(docs/compatibility-gate.md).

## 1. Connection and identity

The client binds one repository, one scope, and one credential at connection.
Authorization applies to every call. Resource paths carry no repository or scope
segments. Path identity repeats no connection fact.

`GET /metadata` returns the compatibility tuple, the package identity, and the
authorized facts about the bound repository and scope. It is the sole version
advertisement.

## 2. Route patterns

The catalog defines the closed collection set, the permitted methods, the type
schemas, the actions, the queries, the preconditions, the receipts, the paging
rules, and the statuses. These patterns apply once, generically:

- `POST /{collection}` creates one typed resource.
- `GET /{collection}` lists resources. Filters and queries are GET parameters.
- `GET /{collection}/{id}` reads one typed resource.
- `PATCH /{collection}/{id}` changes the editable fields, including
  relationships.
- The surface has no DELETE method. Removal from an editable set is a PATCH.

Collections:

- Graph: `sources`, `requirements`, `resolutions`, `rules`, `domains`,
  `boundaries`, `topics`, `questions`.
- Draft and ideation: `contributions`, `synthesis-packets`, `proposals`,
  `verification-runs`, `verification-bindings`.
- Read-only scope indexes: `discussion-containers`, `messages`, `assertions`,
  `dispositions`. An index entry carries the canonical parent address.

Subresources have parent-owned addresses:

- `GET /requirements/{id}/document` reads the assembled document.
- `GET /requirements/{id}/history[/{entry_id}]` reads immutable outcomes.
- `GET /requirements/{id}/history/{entry_id}/evidence/{side}` reads a
  before or after snapshot span.
- `GET /rules/{id}/evidence` reads implementation, verification, and
  Requirement-change evidence.
- `GET|POST /{collection}/{id}/discussions` lists or starts concerns for the six
  supported parent kinds.
- `GET|PATCH /{collection}/{id}/discussions/{discussion_id}` reads a concern or
  changes its permitted status.
- `GET|POST /{collection}/{id}/discussions/{discussion_id}/messages[/{message_id}]`
  lists or appends messages; the GET also supports one message ID.
- `GET /{collection}/{id}/discussion-containers/{container_id}/legacy-messages[/{message_id}]`
  preserves unassigned history. It invents no concern membership. Existing
  container and message identities stay readable.
- `GET|POST /proposals/{id}/assertions` and `GET|POST /proposals/{id}/dispositions`
  expose immutable proposal facts. Immutable facts support creation and reads
  only. Derived verification bindings support creation through their one write
  path and reads only.

Actions are POSTs on a resource path with one declared action name:

- Topics: `claim`, `release`, `close`. Questions: `claim`, `release`, `answer`.
- Requirement review: `POST /requirements/{id}/submit`, and
  `POST /requirements/{id}/submissions/{proposal_id}/decide|withdraw`.
  The submission identity is the existing Proposal ID.
- Verification: `POST /verification-runs/begin-verification`, and
  `POST /verification-runs/{run_id}/complete-verification`. Begin retains
  Rule-ID and declaration-reference targeting.

Queries are GETs with query parameters on their owning path. The query name is a
declared parameter. The closed query set is `search`, `stale`, `impact`, `trace`,
`neighbors`, `resolve-symbol`. No query has its own route.

Three computation exceptions stay collection POSTs:

- `POST /statement-checks` and `POST /authoring-plans` return computed results
  with `MUTATES=false`.
- `POST /authoring-changes` applies the typed specification with `MUTATES=true`.

These exceptions imply no new persistent plan records, no generic action
dispatch, and no weaker authoring ownership checks.

Known carve-out, to be reconciled in a later phase: typed-spec apply
(`POST /authoring-changes`) and full-scope import still replace un-enrolled
graph state without journal entries, while enrolled state takes every change
through the guarded journal.

Receipts stay internal. A write returns success or an error. The client does not
replay a write, and no client-side uncertain-write ledger exists. The journal
resolves an interrupted publication inside the Store. The legacy receipt
operations have no public replacement.

Requirement writes use one guarded path. The review layer is the editing
interface. `POST /requirements` creates a Requirement. `PATCH /requirements/{id}`
edits one under its ETag and enrollment preconditions. No parallel write surface
exists. The read side merges the same way: `GET /requirements/{id}` returns one
resource read carrying the edit state, the relationships, the ETag, and the
decision state together, so the separate edit-state and decision-state reads of
the #273 review surface have no successor.

## 3. Envelope

Every response uses `{data, meta}` or `{error, meta}`. A list uses `data.items`.
MCP uses the same envelope as HTTP. No surface returns a raw array, a flattened
result, or an MCP-only wrapper. `meta` carries the response machinery kept
unchanged from the current surface — stamps, freshness policy, the HMAC-bound
cursor, and limits — per the plan's kept-unchanged list.

A body-bearing request uses `{data}`. HTTP binds catalog-defined controls to
headers. MCP and native calls use the same typed controls. `Idempotency-Key`,
`If-Match`, and the response `ETag` map to Store-supported identity and
precondition checks. The surface never advertises an unsupported guarantee.

## 4. PATCH semantics

PATCH applies a partial delta. A single-field change, an add, or a remove is a
valid PATCH. Omission means unchanged. `null` clears a nullable field. An array
supplies that field's complete final set. The journal holds the canonical final
set.

PATCH never creates a missing resource. PATCH preserves the replacement
eligibility checks. Relationship edits belong to the owning resource's PATCH; no
relationship has its own URL, endpoint, add or remove command, or edge identity.

Text and relationship changes in one PATCH validate and publish atomically. The
Store adapter enforces the atomic final-set validation and the journal
publication. A transport handler never implements a relationship
read-modify-write loop. Required sets, target kinds, cycles, ownership, and
enrollment rules validate against the final resource state.

## 5. Failure statuses

A failure variant declares its status with the variant. The declared statuses of
an operation are its advertised statuses. The runtime returns no undeclared
status. The declared set omits no status that the variants produce.
Under-declaration is impossible.

The base status set is 400, 401, 403, 404, 500, 503. A mutating operation also
declares 409. A read POST declares `MUTATES=false`. The generator linter checks
the live OpenAPI document. It requires the base set, 409 on a mutating route,
and a declared status for each runtime failure variant.

## 6. Compatibility tuple and gate

One authoritative compatibility tuple contains `wire`, `state`,
`review_journal`, and `read_derivation`. `/metadata` is its sole public
advertisement. Generated constants derive from that one definition. A client
supports exactly one tuple and refuses a mismatch at connection.

The release removes versioned URLs, per-call version fields, response version
echoes, old operation aliases, and compatibility translators.
Phase 2b removed the legacy versioned operation wire.

A breaking change requires human authorization. Agents never bump a version, a
release, or the tuple. The required compatibility gate fails when a watched
value changes without an `AUTHORIZED-BUMP` marker written by a human. The gate
binds the marker to the same change as the bump, so a stale marker authorizes
nothing (docs/compatibility-gate.md).

## 7. Grammar

Collections use plural lowercase kebab-case. Payload fields use snake_case, with
closed schemas and no alternate wire aliases. Path parameters use snake_case and
declare their resource. Subresource addresses name the parent resource and the
parent-owned child, never an edge.

The catalog declares ownership, cardinality, allowed methods, action and query
names, mutation classification, preconditions, receipts, paging, and statuses.

The generator linter rejects repository or scope path prefixes, relationship
routes, legacy verb routes, duplicate bindings, unresolved path parameters,
generated-name collisions, GET bodies, required-null requests, raw arrays,
flattened envelopes, MCP-only wrappers, undeclared actions, mutating GETs, read
POSTs without an explicit `MUTATES=false`, and missing status declarations.

Generated clients and MCP expose the collapsed contract. They do not reproduce
the 90 legacy commands under new names. MCP tool descriptions stay
resource-focused and useful.

`payload-identity-repetition` rejects a payload that repeats a connection or
path identity fact: repository, scope, collection, or resource ID.
`tool-description-usefulness` requires each MCP tool description to identify
its resource behavior. The generator checks both rules against the live
OpenAPI and MCP documents.

## 8. Decision record — 2026-09-13 (binding)

1. Requirements have one guarded write path. The review layer is the editing
   interface, not a parallel feature.
2. Relationships are PATCH fields. No relationship surface exists. Partial
   nondestructive deltas are allowed; the full set stays canonical in the
   journal.
3. Addressed Discussions extend to all six parent kinds. No capability retires.
4. A write returns success or an error. No receipt-replay client workflow and no
   uncertain-write UX exist in this contract. Journal publication recovery stays
   internal to the Store. The SDK uncertain-write ledger is dropped.
5. Agents never bump versions, releases, or compatibility values without express
   human authorization. A required check enforces this; a standing Rule repeats
   it.
6. The restructure lands before W2 starts. No parallel web work uses candidate
   artifacts. Queries are GETs with query parameters on their owning path; no
   `/query/{name}` subroutes exist. This rule supersedes the draft
   `GET /query/{query}` family.

## 9. Permanent enforcement

The generator checks the live OpenAPI and MCP documents against this grammar.
It rejects a prohibited route shape, request shape, response envelope, status,
payload identity, or tool description before it writes generated output. The
compatibility gate in `docs/compatibility-gate.md` remains independent and
required.
