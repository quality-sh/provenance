# Effect HTTP SDK

`@quality-sh/provenance/effect` supplies all HTTP operations, runtime wire schemas,
and `ProvenanceApi`, the generated Effect HttpApi definition. Install
`effect@4.0.0-rc.113` in the application. The SDK declares this exact version as
an optional peer. The Promise entry, `@quality-sh/provenance/client`, does not
import Effect and does not require its installation.

The package artifact contains the generated output. Consumers do not run Rust,
run the generator, start an engine, or download schemas. The existing package
arrangement remains in place. The browser package split belongs to
`provenance-cftl`. This entry does not establish that a new release is available.

## Connect and call

```ts
import * as Effect from 'effect/Effect';
import * as Layer from 'effect/Layer';
import { EffectHttpClient, ProvenanceClient } from '@quality-sh/provenance/effect';

const client = await Effect.runPromise(EffectHttpClient.connect({
  baseUrl: 'http://127.0.0.1:42069',
  bearer: sessionToken,
}));
const service = Layer.succeed(ProvenanceClient, client);
const context = { repository: 'configured-target', scope: 'default' };
const read = client.get({ context, request: { node_type: 'requirement', id: 'req_review' } });
const result = await Effect.runPromise(read);
```

The application supplies the endpoint and any bearer credential. A repository
name in `context` is a configured host target, not a URL or a local file path.
Creating a call sends no request. Each execution takes a JSON snapshot of the
input and sends one request. The server retains authority over input refusals,
authorization, revisions, graph rules, and persistence. The SDK does not fill
input defaults or change null values. A value that cannot be serialized fails
with `InvalidRequestError` before dispatch.

Connection checks `/metadata` before it supplies a client. Invalid host URLs,
credentials that cannot form a header, and failed connections expose no private
cause text. Bearer credentials remain at the configured origin. HTTP redirects
are refused. Responses are limited to 16 MiB and pass the shared wire validators.
Malformed success and failure bodies do not enter typed operation results.

`ProvenanceClient.layer(options)` can establish the connection inside an Effect
runtime. For mutation use, retain the resulting client for the whole application
session. The explicit connection above makes that lifetime clear.

## Application-owned atoms

Use `Atom.runtime` with the same client instance:

```ts
import { Atom } from 'effect/unstable/reactivity';

const runtime = Atom.runtime(service);
const document = runtime.atom(Effect.flatMap(ProvenanceClient, sdk =>
  sdk.readDocument({ context, request: { id: 'req_review', limit: 50 } })
));
const update = runtime.fn((statement: string) =>
  Effect.flatMap(ProvenanceClient, sdk => sdk.updateRequirement({
    context,
    request: { scope_id: context.scope, id: 'req_review', statement },
  }))
);
```

The application owns the registry, atom lifetime, cache, invalidation, document
selection, and presentation. A mutation belongs in a function atom invoked by
an explicit action. A read atom can run again when the application refreshes it.
Do not put a mutation in an automatically refreshed read atom.

`ProvenanceApi` describes the wire contract. Constructing a client directly with
`HttpApiClient` or `AtomHttpApi` is not the supported SDK connection path.
`AtomHttpApi` constructs its own client and has different transport and schema
failure handling. It does not inherit the SDK handshake, credentials, response
bounds, or unresolved-write memory. Use the shared service above for network
calls. Exporting the contract does not configure a connection.

## Failures and interruption

Errors have a discriminant named `_tag`. Each generated method has its declared
failure family in `OperationError.failure` and `UncertainWriteError.failure`.
Use `Effect.catchTag`, `Effect.match`, or `Effect.exit` to inspect outcomes.

| Outcome | Meaning |
| --- | --- |
| `ProtocolMismatchError` | The metadata version is incompatible. No operation was sent. |
| `OperationError` | A validated refusal, including stale and cursor refusals. |
| `ConnectionError` | A connection or read transport failure. |
| `MalformedResponseError` | A read response fails the wire contract. |
| `UncertainWriteError` | The host reports an uncertain write, or a dispatched mutation has no confirmed outcome. |
| `InvalidRequestError` | The input cannot be serialized. No operation was sent. |
| `WriteCapacityError` | All 128 unresolved-write slots are occupied. No new mutation was sent. |

Interrupting a read aborts the request and waits for response cleanup. The SDK
has no automatic operation retries. Do not attach a retry policy to a mutation
unless independent evidence establishes that another attempt is appropriate.

A fiber interruption or an application timeout can replace the typed result
with an interruption or timeout cause. It cannot establish rollback. Before
mutation dispatch, the client creates a pending attempt. Response loss,
malformed responses, or interruption leave that attempt uncertain. A confirmed
success or a validated refusal removes it. `client.unresolvedWrites()` returns
copies containing only the attempt ID, operation name, and pending or uncertain
state. `UncertainWriteError.attemptId` identifies the same attempt when that
error reaches the caller. Requests and credentials are not stored in this list.

After any abandoned mutation, inspect this list even if the atom is gone. Keep
the client instance while entries remain unresolved. The list is session memory;
it is not a durable receipt and does not survive a page reload. Resolve an entry
with `client.resolveWrite(id)` only after the application obtains independent
evidence. A pending entry cannot be removed. At capacity, mutation admission
stops without evicting entries. Reads remain available to obtain evidence.

The SDK does not refresh a document after a successful mutation. A later refresh
failure does not change the confirmed mutation result.

## Schemas and cursor reads

Each top-level request, success, and failure schema has a named runtime export,
for example `ReadDocumentRequestInput` and `ReadDocumentSuccessOutput`. Their
Effect `Schema.declare` validators preserve the exact JSON value on decode and
encode. They reject missing required nullable fields, invalid alternatives,
and violated constraints. Defaults remain annotations in the authoritative
OpenAPI document and are applied by the server, not by the client. These opaque
wire schemas do not provide field-level schema construction or a replacement
OpenAPI exporter. The Rust exporter remains the authority.

The official Effect generator supplies the type declarations and HttpApi. Its
default object decoder accepts or strips extra fields. The generated runtime
schema declarations therefore use the existing standalone Ajv compiler for
exact acceptance and identity preservation. Response validators are shared with
the Promise client. Additional generated validators cover request schemas.
There is no browser compiler or schema download.

Nested wire types remain available through `components['schemas']` from the
same entry. Do not copy those declarations into an application.

Search and document calls return their full stamp and opaque cursor. Send the
same selector and returned cursor for continuation. An empty or short page can
still have `next_cursor`. The SDK does not accumulate pages, infer completeness,
or restart a refused continuation. Only a terminal sequence at one revision
establishes completeness. See [cursor reads](cursor-reads.md).

## Integration handoff

The merged web adapter at
[`8f78d635280cf3cd6e4f2a1bf6dc06d3fe463f0d`](https://github.com/quality-sh/provenance-web/commit/8f78d635280cf3cd6e4f2a1bf6dc06d3fe463f0d)
remains a useful integration reference. Its target types preserve repository,
scope, record kind, ID, and observed ownership. Its update mappings translate
UI null values to `clear_fields`; SDK null retains the existing omission
semantics. Reuse those mappings with the corresponding generated Effect calls.
Keep its confirmed-save, failed-refresh, generation, and pending-intent tests.
The Promise adapter is not the final Effect runtime integration.

Integration requires an SDK release and consumer dependency and lockfile updates.
CLI asset composition also needs a compatible renderer archive and checksum pin.
Downstream work must supply one Effect version, connect the shared service to
application atoms, and keep the client alive while mutations are unresolved.
Review-save guards and durable receipts are not added by this SDK. Existing
review controls must not gain authority merely because a CRUD call exists.
Full UI migration, review lifecycles, the browser package split, release
publication, and renderer composition remain separate work. Web PR 15 and W1
are not reopened by this change.

The pinned Effect release has unresolved names in its published declaration
files. TypeScript consumers use `skipLibCheck: true`; application and generated
source remain checked in strict mode. One installed browser consumer checks the
shared package contract used by both browser owners. Its bundle executes without
Node globals and checks host reads, writes, atoms, and a persisted mutation whose
response is lost. A separate Promise consumer checks installation without Effect.
