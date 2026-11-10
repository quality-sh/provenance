# Effect HTTP SDK

`@quality-sh/provenance/effect` supplies the generated Effect operations, wire
schemas, and `ProvenanceApi`. Install the exact Effect peer version that the
package manifest declares. The Promise entry at
`@quality-sh/provenance/client` does not import Effect.

## Connect and call

Bind the endpoint, credential, repository, and scope once:

```ts
import * as Effect from "effect/Effect";
import { EffectHttpClient } from "@quality-sh/provenance/effect";

const client = await Effect.runPromise(EffectHttpClient.connect({
  baseUrl: "http://127.0.0.1:42069",
  bearer: sessionToken,
  repository: "configured-target",
  scope: "default",
}));

const requirement = await Effect.runPromise(
  client.getRequirement({ id: "req_review" }),
);
```

The connection reads `GET /metadata`. It checks the complete compatibility
tuple and the optional repository and scope pins before it returns a client.
The credential stays at the configured origin. Redirects are refused and
responses are limited to 16 MiB.

Each method follows the resource catalog. Request bodies use `data`, path and
query values use typed method fields, and list results use `data.items`:

```ts
const rules = await Effect.runPromise(client.listRules({
  query: "search",
  text: "time bounded",
  limit: 20,
}));

const trace = await Effect.runPromise(client.getRule({
  id: "rule_expiry",
  query: "trace",
  direction: "out",
  max_depth: 2,
}));
```

`ProvenanceClient.layer(options)` creates the same connection as an Effect
service. Retain one client for the application session.

## Atoms

Use the shared service in application-owned atoms:

```ts
import * as Effect from "effect/Effect";
import { Atom } from "effect/reactivity";
import { ProvenanceClient } from "@quality-sh/provenance/effect";

const runtime = Atom.runtime(ProvenanceClient.layer({
  baseUrl,
  bearer,
  repository,
  scope,
}));

const document = runtime.atom(Effect.flatMap(
  ProvenanceClient,
  sdk => sdk.getRequirementDocument({ id: "req_review", limit: 50 }),
));
```

The application owns atom lifetime, invalidation, and presentation. Invoke a
mutation from an explicit action.

## Failures and interruption

Errors use `_tag` as their discriminant:

| Outcome | Meaning |
| --- | --- |
| `ProtocolMismatchError` | The compatibility tuple differs. No operation was sent. |
| `IdentityMismatchError` | Metadata names a different repository or scope. |
| `OperationError` | The host returned a validated typed refusal. |
| `ConnectionError` | The connection or response stream failed. A write was not sent, or the operation is a read. |
| `OutcomeUnknownError` | A write possibly went to the host, and its response did not arrive. The outcome is not known. |
| `MalformedResponseError` | A response does not match the operation contract. |
| `InvalidRequestError` | The input cannot be serialized. |

The clients do not retry mutations and keep no uncertain-write ledger. A write
fails with `OutcomeUnknownError` when the request possibly went to the host and
its response did not arrive. For example, the connection closes before the
response, the response stream fails, or the host sends a redirect. A Promise
client call also gives this error when the caller aborts its signal after
dispatch. The write possibly changed the state. Read the current
state of the addressed resource before you send the write again. A failure
before dispatch, such as a refused connection or a signal that is aborted before
the request starts, is a `ConnectionError`. The client reports a write as not
sent only when the failed system call is the connect or the name lookup, or when
the connect step timed out. A failure after the response headers arrive is never
a failure before dispatch. A read that loses its response is also a
`ConnectionError`. The Store resolves an interrupted publication.

When the client cannot identify the cause of a transport failure, such as a
`TypeError` from a browser `fetch`, it cannot show that the write was not sent.
The write then fails with `OutcomeUnknownError`.

Interrupting an Effect aborts the request and waits for response cleanup. A
fiber interruption can replace a typed result with an interruption cause. An
interrupted write gives an interruption cause, not `OutcomeUnknownError`, but its
outcome is also not known. Read the current state before you send it again.

## Schemas

The generated contract exports each request, success, and failure schema by its
operation name. It also exports matchers for discriminated unions in the new
failure families. The runtime validators preserve the exact JSON value and
reject malformed envelopes. No runtime compiler or schema download is needed.

The package includes generated output. Consumers do not run Rust, run the
generator, or start an engine process.
