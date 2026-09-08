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
  Disjoint, required `kind` tags permit an equivalent `anyOf` to `oneOf`
  conversion so Typify emits a closed error enum. Overlapping tags do not
  permit this conversion.
  OpenAPI retains numeric bounds and defaults. Typify can enforce a bound
  through its scalar choice, such as a nonzero integer. The shared handler
  enforces remaining bounds. Tests cover local zero rejection, HTTP zero
  refusal, and the named method's refusal for 201.
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
node tools/operation-codegen/test-clients.mjs records
node tools/operation-codegen/generate.mjs --check
```

The candidate test compiles both languages against production shapes. It checks
nullable required fields, omitted fields, flattened results, tagged variants,
and issue 9. It then round-trips actual Rust wire values through generated Rust
types in a temporary crate. The real-host script tests both named clients and
waits for the test host to stop. This listener requires the transport test-fixture
feature; it is not a production listener. The records fixture selects opaque
repository names, exercises separate scopes and freshness policies, and uses a
fixed test bearer credential. Both clients preserve each method's declared
failure family.

`--check` writes to a temporary directory and compares the full file inventory
and file bytes. Missing, stale, changed, and oversized files fail. Tests mutate
temporary copies to verify each condition. The checkout remains unchanged.

Clients require an HTTP host and check its protocol version before use. They do
not start an engine process. They do not retry operation calls or follow redirects.

`connectWithBearer` in TypeScript and `connect_with_bearer` in Rust authenticate
both metadata and operation requests. The Rust error contains a named
`OperationFailure` variant for the method that refused the request. The
TypeScript error contains a union of the operation failure envelopes.

The generated Rust module allows only Typify's `if_not_else` conversion style
and explicit default helpers (`missing_const_for_fn`, `derivable_impls`).
Handwritten code keeps the normal workspace lint settings.
