# Operation generation

The Store catalog owns each operation's request, result, and failure types. The
exporter uses request schemas in the deserialize direction and result schemas in
the serialize direction. It gives each component an operation and direction
prefix. Duplicate component names stop generation.

OpenAPI 3.1 is the input for both HTTP clients. MCP uses the catalog's input
schema helper, which the transport also uses. The fixture document contains real
production types; its operations are not registered or exposed by Phase 1.

Tools are pinned in Cargo.lock and this directory's package-lock.json:

- openapi-typescript 7.13.0 generates TypeScript types.
- typify 0.7.0 generates Rust types. The exporter converts JSON Schema `const`
  to a one-value `enum` and rewrites references for its draft-7 input reader.
  Request numbers retain the native request type's validation boundary:
  the shared handler checks semantic bounds. OpenAPI retains min/max and
  defaults; generated Rust checks scalar types and null handling. It does
  not add range checks that native request deserialization does not have.
- Checked source templates generate named HTTP methods from OpenAPI operation IDs.

Each generated Rust file contains one model and its implementations.
Generation removes Typify's repeated schema doc blocks; OpenAPI keeps the schema.
A model group over 500 lines stops generation. Generated output is never edited.

Run `npm ci --prefix tools/operation-codegen`, then:

```sh
node tools/operation-codegen/generate.mjs
cargo test -p provenance-codegen
node --test tools/operation-codegen/*.test.mjs
cargo test -p provenance-http-client
node tools/operation-codegen/test-clients.mjs statements
node tools/operation-codegen/generate.mjs --check
```

The candidate test compiles both languages against production shapes. It checks
nullable required fields, omitted fields, flattened results, tagged variants,
and issue 9. It then round-trips actual Rust wire values through generated Rust
types in a temporary crate. The real-host script tests both named clients and
waits for the test host to stop. This listener requires the transport test-fixture
feature; it is not a production listener.

`--check` writes to a temporary directory and compares the full file inventory
and file bytes. Missing, stale, changed, and oversized files fail. Tests mutate
temporary copies to verify each condition. The checkout remains unchanged.

Clients require an HTTP host and check its protocol version before use. They do
not start an engine process. They do not retry operation calls or follow redirects.
