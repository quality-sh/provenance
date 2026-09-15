# Operation contract

The catalog in `provenance-store` defines the public API. Each definition binds
an HTTP method, a resource path, an operation ID, a mutation classification,
parameters, request and response schemas, failure statuses, and one Store
operation. HTTP, MCP, the generated CLI dialect, and generated clients derive
from this catalog.

## Connection identity

A connection binds one repository, one scope, and one credential. Calls do not
repeat these facts in a path or request body. `GET /metadata` returns the single
compatibility tuple, package identity, contract digest, and the repository and
scope that the connection can use. It is the only public version advertisement.
The tuple does not change as part of the v2 route restructure.

## Resources

The writable graph collections are `sources`, `requirements`, `resolutions`,
`rules`, `domains`, `boundaries`, `topics`, and `questions`. Draft collections
are `contributions`, `synthesis-packets`, and `proposals`. Verification uses
`verification-runs` and `verification-bindings`. The read-only indexes are
`discussion-containers`, `messages`, `assertions`, and `dispositions`.

Collections use these patterns when the catalog declares the method:

```text
POST  /{collection}
GET   /{collection}
GET   /{collection}/{id}
PATCH /{collection}/{id}
```

A PATCH supplies a partial delta. An omitted field is unchanged, `null` clears
a nullable field, and an array is the complete final value of that field.
Relationship changes use the owner PATCH. There are no edge routes and no
DELETE routes.

A list response puts records in `data.items`. A member response puts the record
in `data`. Queries use GET parameters. `search`, `stale`, and `resolve-symbol`
use the collection path. `trace`, `neighbors`, and `impact` use the addressed
member path. No query has a separate URL.

## Parent-owned resources

The catalog supplies these parent-owned reads:

```text
GET /requirements/{id}/document
GET /requirements/{id}/history
GET /requirements/{id}/history/{entry_id}
GET /requirements/{id}/history/{entry_id}/evidence/{side}
GET /rules/{id}/evidence
```

Sources, Requirements, Resolutions, Rules, Topics, and Questions own addressed
Discussions:

```text
GET|POST  /{collection}/{id}/discussions
GET|PATCH /{collection}/{id}/discussions/{discussion_id}
GET|POST  /{collection}/{id}/discussions/{discussion_id}/messages
GET       /{collection}/{id}/discussions/{discussion_id}/messages/{message_id}
GET       /{collection}/{id}/discussion-containers/{container_id}/legacy-messages
GET       /{collection}/{id}/discussion-containers/{container_id}/legacy-messages/{message_id}
```

A Proposal owns immutable assertion and disposition facts:

```text
GET|POST /proposals/{id}/assertions
GET      /proposals/{id}/assertions/{fact_id}
GET|POST /proposals/{id}/dispositions
GET      /proposals/{id}/dispositions/{fact_id}
```

## Actions and computations

Topic actions are `claim`, `release`, and `close`. Question actions are
`claim`, `release`, and `answer`. Requirement review uses these routes:

```text
POST /requirements/{id}/submit
POST /requirements/{id}/submissions/{proposal_id}/decide
POST /requirements/{id}/submissions/{proposal_id}/withdraw
```

Verification uses:

```text
POST /verification-runs/begin-verification
POST /verification-runs/{run_id}/complete-verification
```

The three computation routes are collection POSTs. Statement checks and
authoring plans declare `x-operation-mutates: false`. Authoring changes declare
`x-operation-mutates: true`.

```text
POST /statement-checks
POST /authoring-plans
POST /authoring-changes
```

## Envelopes and failures

A body-bearing request uses `{data}`. Every successful response uses
`{data,meta}`. Every failure uses `{error,meta}`. HTTP and MCP use the same
envelope. `meta` carries the projection stamp, freshness information, page
limit, `has_more`, and the opaque next cursor when those fields apply.

The base failure status set is 400, 401, 403, 404, 500, and 503. A mutating
operation also declares 409. The catalog adds operation-specific statuses from
its typed failure family. The OpenAPI responses come from this declared set.
The runtime uses the status of the typed failure variant.

HTTP checks the credential, Host, Origin, admission state, and body size before
it invokes a Store operation. The connection controls repository and scope.
The transport binds `Idempotency-Key` and `If-Match` only where the Store
supports receipts or preconditions. A successful supported resource response
carries `ETag`.

A write has one terminal client result: success or a typed error. Clients do not
keep an uncertain-write ledger and do not retry a mutation. The Store resolves
an interrupted journal publication. Receipt reads remain internal.

## Generation and role projection

The OpenAPI document, operation IDs, `x-operation-mutates`, MCP tools, and the
Promise, Effect, and Rust clients derive from the registered resource surface.
MCP descriptions identify the resource behavior. Writable hosts expose all
permitted tools. Read-only hosts omit mutating tools.

Generated clients refuse redirects, cap response bytes, validate the full
response envelope, and compare the complete compatibility tuple during
`GET /metadata`. They never start a host or select another repository or scope
for one call.

The legacy operation inventory remains in
`tools/operation-codegen/legacy-operation-coverage.json` until Phase 3. It maps
all 90 former operations to the v2 patterns exactly once. It does not expose
legacy aliases.
