# Operation contract

The operation catalog in `provenance-store` binds each operation name to its
request, result, failure family, resource needs, and handler. Native calls use
Rust types. HTTP and MCP adapters use the same handler and preparation path.
The native Rust SDK remains available.

The catalog contains `check-statement`, `info`, `get`, `search`, `neighbors`,
`trace`, `impact`, `resolve-symbol`, `evidence`, `stale`, `verification-runs`,
`verification-bindings`, `plan`, `apply`, `begin-verification`,
`complete-verification`, `create-source`, `create-requirement`,
`create-resolution`, `create-rule`, `add-source-reference`, `list-threads`,
`list-messages`, `post-thread-message`, `list-proposals`, `list-dispositions`,
`list-assertions`, `create-proposal`, `create-assertion`, and
`create-disposition`. Additional edits and native actions are listed in
[record updates](record-updates.md). The statement
handler returns the existing ASD-STE100 analyzer report. A finding is a successful report result.
The operation does not open a repository, load settings, or use a dictionary.

## Statement calls

HTTP uses `POST /v7/operations/check-statement` with a JSON body:

```json
{"request":{"statement":"Stop; wait."}}
```

MCP uses the `check-statement` tool with these arguments:

```json
{"protocol_version":7,"call":{"request":{"statement":"Stop; wait."}}}
```

Both calls return the full report, including integer `issue: 9` and UTF-8 byte
spans. Extra fields, including repository context, are invalid. The native CLI
keeps its existing stdin body, `{"statement":"Stop; wait."}`, and output bytes.

The repository-free `/metadata` endpoint reports the engine and operation
protocol versions. Each operation call also checks its dispatch version.
Unknown operations do not claim an executed operation name in their failure.
Wire failures contain typed causes. Native errors retain native diagnostics.

## Selected graph reads

`info` requires only a configured target identifier:

```json
{"context":{"repository":"first"},"request":{}}
```

The external result retains engine, operation-protocol, and state-schema
versions. Its `repository` is `first`. Native `engine_info` keeps the resolved
repository path. Neither result carries a projection stamp.

The eight structured reads require a target and scope. They accept the existing
query fields and an optional freshness setting:

```json
{"context":{"repository":"first","scope":"default","freshness":"catch_up"},"request":{"node_type":"rule","id":"rule_shared"}}
```

An absent or null freshness setting uses `ReadPolicy::resolve`: the request
setting precedes the repository setting, which precedes the built-in default.
`catch_up` includes saved graph edits. `annotate_only` reads the stored
projection. `refuse_stale` refuses a changed projection. Every successful
structured read retains its full stamp and operation identity. Pages remain
bounded at 200, with `has_more` and no cursor.

External validation uses the same semantic checks as native queries, but runs
before host preparation. Native queries retain validation after the freshness
step. The native CLI uses registered entries for these eight reads and retains
its existing JSON and stderr behavior.

## Read diagnostics

A stale refusal has status 409 and retains `serial`, `digest`, `instance_id`,
and each moved unit's logical name and stored/live digest. An empty stored
digest identifies a new unit; an empty live digest identifies a departed unit.
`no_projection`, `schema_behind`, `half_migrated`, and `unit_unreadable` also
have status 409. The unreadable-unit refusal retains its logical unit name.
An unknown scope has status 404; an unclassified read failure has status 500.

The external projection omits native database paths, unreadable-file paths,
and lower-level error text. Native errors keep those details. When catch-up
fails but a stored projection can answer, the external result keeps the actual
stamp with `policy: "catch_up_failed"`, adds
`freshness_cause: "catch_up_failed"`, and uses this fixed message:
`catch-up failed; answer uses the stored projection`. The cause identifies the
failed stage; it does not infer a more specific cause from private error text.
No diagnostic projection changes or invents a stamp.

## Authoring and verification

`plan` and `apply` accept the existing typed declaration document. Their context
contains the configured repository identifier and scope, without freshness:

```json
{"context":{"repository":"first","scope":"default"},"request":{"schema_version":2,"spec":"example","declared_by":"spec://example","requirements":[{"key":"saved","statement":"The request preserves the saved graph."}]}}
```

`plan` returns proposed changes and ownership conflicts. `apply` publishes the
declarations and their required relationships. Both use the same identity,
adoption, source-alias, and statement checks as native authoring. A string field
does not permit an unknown source kind or verification method; the shared
handler also validates these values.

`begin-verification` materializes the durable binding, creates a running run,
and clears the applicable Requirement reviews. `complete-verification` records
the passed or failed result. These operations have the same context as `apply`.
The local callback stays in the SDK; it never crosses the wire. A host commit
pin identifies host file state and does not attest remote callback execution.

Implementation and verification files use portable repository-relative paths.
The host uses the held-file checks described in
[repository evidence access](operation-file-access.md). A caller path grants
no access. Local SDK file inference must use a separately configured local root.

## Existing creation and Source attachment

The five creation and attachment operations accept the existing Store input
fields and return the existing Source, Requirement, Resolution, or Rule record.
The request retains `scope_id`; it must equal the scope in the authorized call
context. The operation does not replace it with a default.

Creation retains the caller's existing placement and origin fields. Native
reference checks, ID ordering, duplicate rejection, commit-pin checks, statement
checks, and lifecycle values apply. Origin fields retain native semantics;
these operations do not infer or validate message membership. Resolution audit
fields describe the supplied record and do not authenticate a human actor.

`add-source-reference` attaches a Source to a Requirement through its existing
`source_refs` field. The same Source and clause are not added twice. Each call
uses one existing Store operation. Several calls do not form one transaction.

No unified discussion outcome, fuzzy matching, default placement policy,
edit history, content journal, or new persisted relationship is added. All
repository listeners remain explicit isolated fixtures pending production
access control.

## Existing discussions

`list-threads` and `list-messages` use the selected repository and scope with a
`null` request. They return the complete native record arrays in native order.
MCP wraps each array in `result`, as it does for the other list operations.

`post-thread-message` accepts the existing `scope_id`, `parent`, `role`, and
`body` fields. The request scope must equal the authorized context scope.
The result contains the existing `thread` and `message` records. A parent can
be a Source, Requirement, Resolution, Rule, Topic, or Question. Domain and
Boundary parents remain unsupported. Native posting does not require the
parent record to exist. Message role is record data, not caller authority.

Posting selects the canonical active Thread and archives its active siblings.
If no active Thread exists, it creates a new Thread and retains terminal
history. Body text is preserved, but a blank body is refused. Thread and
Message IDs, timestamps, fields, and storage paths retain native behavior.
The Thread is published before the Message; a failure after that first
publication reports `uncertain_write`, including a Message shard read failure.

These operations add no separate reply groups, message membership, resolve or
reopen action, new parent kind, or proposal association. Native CLI and Rust
calls remain direct and do not need an HTTP server. Generated SDKs only call
an existing host and do not start or manage it.

## Proposal lifecycle records

`list-proposals`, `list-dispositions`, and `list-assertions` use the selected
repository and scope with a `null` request. They return the complete native
record arrays in native order. `list-proposals` returns the validated native
effective-state projection: each row reports the state that recorded
assertions and dispositions reach, not the state the row claims. The stored
definition stays immutable. MCP wraps each array in `result`, as it does for
the other list operations.

`create-proposal`, `create-assertion`, and `create-disposition` accept the
existing Store input fields and return the existing ProposalCard,
AssertionRecord, or DispositionRecord. The request retains `scope_id`; it must
equal the scope in the authorized call context. The operation does not replace
it with a default.

These operations call the existing writers. Each write takes the repository
publication lock and then the scope lifecycle lock, and keeps the native
validation order: aggregate validation, the disposition write gate, the
canonical-artifact check, and single-shard publication. An acceptance rests on
a prior assertion, unless a human actor names a canonical artifact they
ratified; that exception stays as native. The disposition actor is record
data. The manifest `disposition_actor_ids` allowlist and its empty-list
refusal decide who may record a disposition; the call context grants no
authority. A disposition rationale must not be empty. Proposal definitions
and one authoritative assertion and disposition stay immutable.

An unclassified refusal before publication reports `write_failed`. These
writers publish one shard, so a write that starts and fails reports
`uncertain_write`.

The catalog exposes no other native proposal operations. `surface_proposals`,
`land_ideation_batch`, `create_asserted_proposal`, and
`assert_proposal_after_human_decision` stay native-only. The audited Phase 7
and Phase 8 history and content behaviors stay out of scope; they are not
awaiting model approval. Bulk landing and convenience landing stay out of
scope.

## Current operation limits

The catalog exposes existing native operations and typed record updates.
See [record updates](record-updates.md) for fields, clearing, ownership,
shaping actions, and relationship operations. It does not implement
text-edit history linked to discussions or Source-content editing.

### Graph text and discussion history

`plan` and `apply` reconcile typed declarations. Their per-resource `changes`
entries contain `field`, `before`, and `after`, but the report is transient.
These operations do not provide a general record editor or stored edit history.

`StateStore::update_question`
(`crates/provenance-store/src/state_store/shaping_writers.rs`) retains its existing partial-state input and delegates to the shared question
edit writer. The catalog and CLI also expose question text edits and explicit
reference clearing.

`RequirementReview` records
(`crates/provenance-store/src/state_store/requirement_reviews.rs`) retain
before/after statement values for Requirement reviews. A restated Requirement
raises a review per affected Rule, and verification marks it cleared. The
record names no Thread or Message. Creation-time `origin_thread` and
`origin_message` fields do not identify later edits.

No native operation stores the requested link between an edit and a discussion.
Adding that relationship is outside the unchanged data model.

### Source content

A Source record holds citation metadata such as `source_type`, `url`,
`reference`, and `commit_pin`. `create-source`
(`crates/provenance-store/src/state_store/writers.rs`) stores the record; it
does not open the cited target. A citation grants no file or network access.
Structured queries return record fields, not the cited content. Declaration
`apply` can restate or retire a Source but does not edit its cited document.

`evidence`, `impact`, and `resolve-symbol` use the held-file seam for evidence
and scans (`crates/provenance-store/src/operations/files.rs`). Their request
paths are below a configured repository root
([repository evidence access](operation-file-access.md)). Fixture roots do not
map Source citations or grant production access. No native operation maps a
Source citation to an authorized content target or publishes content edits
with linked discussion history.

### Proposal and disposition records

The six proposal-lifecycle operations expose
`StateStore::list_proposal_cards`, `list_dispositions`,
`list_assertion_records` (`crates/provenance-store/src/state_store.rs`), and
the writers in `crates/provenance-store/src/state_store/proposal_writers.rs`.
They preserve the existing inputs, results, validation, and lifecycle locks.
The catalog also exposes native contribution and synthesis-packet creation
and upsert writers. The proposal surfacing projection and ideation landing
batches stay native-only.

## Write failures and task ownership

Protocol, access, and known validation refusals occur before intended graph
publication. Validation can still follow cache or recovery maintenance; a
refusal does not promise that no filesystem byte changed. The wire retains
typed ownership conflicts and statement diagnostics. Native callers retain
their detailed diagnostics.

`apply` can replace several shards. A verification start can publish a binding,
a run, and review changes. The publication lock does not provide rollback.
After publication starts, an error reports `uncertain_write`; inspect saved
state before another submission. This includes a write task that panics after
it starts. An unclassified failure before publication uses `write_failed`.
Neither error is a validation refusal or a promise of crash-atomic writes.

The execution task owns started work through caller disconnection. Shutdown
closes admission and waits for that work. A lost or malformed response can
leave a client unable to tell whether a write completed. Clients must report
that uncertainty and must not retry a mutation automatically.

## Fixture access

Repository host construction is available only with the `test-fixture`
feature and an explicit `FixtureAccess` policy. Each policy has a fixed map
of opaque targets to configured roots, explicit target/scope grants, a
credential, and an expected Host value. Requests cannot supply filesystem
roots. Duplicate or malformed target configuration is refused at startup.
Restart the host to change its configuration.

HTTP checks credentials, Host, and Origin before body decoding. Dispatch
checks framing and versions, resolves the configured target, checks the
fixture grant and allowed scope, then loads read settings. Scope validation
reads only the manifest and does not open a projection or run recovery.
Denied calls leave repository file bytes and directory entries unchanged.
Caller-owned MCP fixture streams represent the configured test principal;
unavailable or denied tools are not advertised and direct selection refuses.

The default host remains data-free. This fixture policy does not implement
production RBAC or authorize a real repository listener. Production repository
exposure remains unavailable pending the approved access implementation.

## Generation

Schema metadata lives beside the Rust wire types behind the `schema` feature.
Requests use deserialize schemas. Results use serialize schemas. The generator
retains omission, null, tag, and flattened-field behavior. Schema checks cover
the actual query, plan, verification, and analyzer types.

The catalog generates the documents in `contracts/operations`. Those OpenAPI
definitions generate the TypeScript and Rust HTTP methods and types. Clients
connect to an existing host. They do not start one or fall back to a subprocess.

Run generation with:

```sh
npm ci --prefix tools/operation-codegen
node tools/operation-codegen/generate.mjs
node tools/operation-codegen/generate.mjs --check
```

Generated files never enter source control. Build and test commands prepare the
ignored output, and release jobs include it in package artifacts. Package consumers
do not need to run the generator. Before a workspace Cargo build, run
`node tools/operation-codegen/ensure-generated.mjs` to prepare current output.

The check generates twice in separate temporary directories and compares contents
and the complete file inventory. Differences fail the check. Generated Rust files
obey the repository's 500-line limit. A pre-commit and CI check rejects generated
paths in the Git index.

## Development host

The listeners are isolated test fixtures. The client test runner starts each
fixture explicitly, waits
for its selected loopback address, runs both clients, and stops the fixture.

```sh
node tools/operation-codegen/test-clients.mjs statements
node tools/operation-codegen/test-clients.mjs records
node tools/operation-codegen/test-clients.mjs evidence
node tools/operation-codegen/test-clients.mjs writes
node tools/operation-codegen/test-clients.mjs creation
node tools/operation-codegen/test-clients.mjs discussions
node tools/operation-codegen/test-clients.mjs ideation
```

The adapters bound request bodies and concurrent work. Blocking operation work
runs outside async executor threads. A disconnected caller does not cancel
started work. Shutdown stops admission and joins started work.

## Compatibility

The operation protocol advances from 6 to 7. The TypeScript SDK uses the
generated HTTP client for the original sixteen operations. The generated client
also exposes creation, attachment, discussion, proposal-lifecycle, and
[record update operations](record-updates.md).
Configure `endpoint`,
`bearer`, `repositoryId`, and `scope`; configure `localRoot` separately when
converting local implementation or verification paths. The SDK rejects the old
implicit repository/subprocess configuration. Both HTTP clients validate
responses against the generated contract and report an uncertain write when
they lose a mutation response or cannot decode it. They never replay the write.

Package installation still supplies the native engine and binary shim. Packed
tests connect explicitly to a source-checkout fixture host; they do not prove a
production repository host is available from the installed package. The host
configuration and access milestones must pass before releasing this migration.

The operation-protocol change does not change legacy disposition grants or
consume their migration window. The later `provenance-cvs` release owns that
window. State schema, read derivation, and `graph-reference-v1` remain separate.

See [repository evidence access](operation-file-access.md) for held-file behavior,
MCP list wrapping, and the control-data trust assumptions. Real repository
hosting remains gated.
