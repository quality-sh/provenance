# Operation contract

The operation catalog in `provenance-store` binds each operation name to its
request, result, failure family, resource needs, and handler. Native calls use
Rust types. HTTP and MCP adapters use the same handler and preparation path.
The native Rust SDK remains available.

The first contract slice contains `check-statement`. Its handler returns the
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

The phase-one listener is an isolated test fixture. It is not a production
repository host. The client test runner starts the fixture explicitly, waits
for its selected loopback address, runs both clients, and stops the fixture.

```sh
node tools/operation-codegen/test-clients.mjs statements
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
