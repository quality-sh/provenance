# Operation contract

The operation catalog in `provenance-store` binds each operation name to its
request, result, failure family, resource needs, and handler. Native calls use
Rust types. HTTP and MCP adapters use the same handler and preparation path.
The native Rust SDK remains available.

The catalog contains `check-statement`, `info`, `get`, `search`, `neighbors`,
and `trace`. The statement handler returns the
existing ASD-STE100 analyzer report. A finding is a successful report result.
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

The four structured reads require a target and scope. They accept the existing
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
step. The native CLI uses registered entries for these four reads and retains
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

The check writes to a temporary directory. It compares both contents and the
complete file inventory. Missing, changed, or stale output fails the check.
Generated Rust files obey the repository's 500-line limit.

## Development host

The listeners are isolated test fixtures. It is not a production
repository host. The client test runner starts the fixture explicitly, waits
for its selected loopback address, runs both clients, and stops the fixture.

```sh
node tools/operation-codegen/test-clients.mjs statements
node tools/operation-codegen/test-clients.mjs records
```

The adapters bound request bodies and concurrent work. Blocking operation work
runs outside async executor threads. A disconnected caller does not cancel
started work. Shutdown stops admission and joins started work.

## Compatibility

The operation protocol advances from 6 to 7. The existing TypeScript transport
and fixtures use 7 during the staged migration. This does not release the new
SDK or remove package-supplied engine installation.

The operation-protocol change does not change legacy disposition grants or
consume their migration window. The later `provenance-cvs` release owns that
window. State schema, read derivation, and `graph-reference-v1` remain separate.
