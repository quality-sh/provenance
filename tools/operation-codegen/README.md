# Operation generation

The Store catalog owns each operation's request, result, and failure types. The
exporter uses request schemas in the deserialize direction and result schemas in
the serialize direction. It gives each component an operation and direction
prefix. Duplicate component names stop generation.

OpenAPI 3.1 is the input for both HTTP clients. MCP uses the catalog's input
schema helper, which the transport also uses. The fixture document contains real
production types; its operations are not registered or exposed by Phase 1.

Tools are pinned in Cargo.lock and this directory's package-lock.json:

- openapi-typescript 7.13.0 generates TypeScript types with
  `defaultNonNullable: false`. Schema defaults do not make input fields required.
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

Each generated Rust model file contains its implementations, including reverse
conversions to primitive types. Directional schema roots declare their model
family in OpenAPI; each family owns an include index. Named operation methods
have separate files, and connection setup remains in the shared client file.
Generation removes Typify's repeated schema doc blocks; OpenAPI keeps the schema.
A model, family index, or other generated Rust file over 500 lines stops generation.
Generated output is never edited or committed.

Before the first workspace build or test, install the pinned Rust toolchain and
run `npm ci --prefix tools/operation-codegen`, then:

```sh
node tools/operation-codegen/generate.mjs
cargo test -p provenance-codegen
npm test --prefix tools/operation-codegen
cargo test -p provenance-http-client
node tools/operation-codegen/test-clients.mjs statements
node tools/operation-codegen/test-clients.mjs records
node tools/operation-codegen/test-clients.mjs evidence
node tools/operation-codegen/test-clients.mjs writes
node tools/operation-codegen/test-clients.mjs creation
node tools/operation-codegen/test-clients.mjs discussions
node tools/operation-codegen/test-clients.mjs ideation
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
failure family. The evidence fixture adds two real Git commits and complete
verification records. Its checks cover all four cuts, null and omitted fields,
list scope and filters, and typed file and Git refusals.

`--check` generates twice in separate temporary directories and compares the full
file inventory and bytes. Differences and oversized files fail. It does not
compare against a source-control snapshot or change the checkout.

The documents and client source directories are ignored. Generation writes an
ignored `.generation.json` receipt with source and output hashes. Run
`node tools/operation-codegen/ensure-generated.mjs` before later workspace builds.
It reuses complete, current output without loading generator dependencies. Missing,
changed, or stale output causes regeneration. The source fingerprint normalizes
checkout line endings so a CI artifact can be used on Windows.

SDK build and test commands prepare generated source automatically. CI and release
jobs generate once and pass the output as a build artifact to dependent jobs.
Published npm and Rust packages include their client outputs; package consumers
do not need Node generation tools or a Rust generator. The release consumer map
`packages/provenance/src/engine-packages.ts` is also generated during SDK builds.
The pre-commit hook and CI reject generated paths in the Git index.
The generator and real-host tests use the executable paths reported by Cargo,
including configured target directories and platform executable suffixes.

Clients require an HTTP host and check its protocol version before use. They do
not start an engine process. They do not retry operation calls or follow redirects.

`connectWithBearer` in TypeScript and `connect_with_bearer` in Rust authenticate
both metadata and operation requests. The Rust error contains a named
`OperationFailure` variant for the method that refused the request. The
TypeScript error contains a union of the operation failure envelopes.

The generated Rust module allows only Typify's `if_not_else` conversion style
and explicit default helpers (`missing_const_for_fn`, `derivable_impls`,
`default_trait_access`). Models with more than three declared Boolean fields
retain those wire fields with a local `struct_excessive_bools` allowance.
Handwritten code keeps the normal workspace lint settings.

Response validators compile from the same OpenAPI components. TypeScript pins
Ajv 8.20.0, ajv-formats 3.0.1 and esbuild 0.25.11 to emit standalone browser ESM;
there is no runtime compiler or schema download. The Rust runtime pins jsonschema
0.18.3 without default network features, rejects external resolvers explicitly,
and compiles validators once into a shared cache. Required nullable fields are
validated before concrete deserialization. Metadata uses the generated shape
with its version const removed only for compatibility classification, then checks
the supported version explicitly.

The TypeScript tool adapter moves reference conjunctions into allOf while keeping
sibling constraints at their evaluation scope. This preserves tagged graph-node
narrowing that the pinned tool otherwise drops. The authoritative OpenAPI and
runtime validation schemas remain unchanged. The Rust union adapter deduplicates
identical tagged alternatives only after proving object-only disjoint tags.

The catalog mutation annotation drives response-loss classification in both
clients. Malformed success or refusal after a write, interrupted connections,
and validated uncertain/internal write outcomes remain uncertain. Response bodies
are bounded to 16 MiB, and public error messages do not include raw bodies.
The write fixture checks plan without mutation, apply, verification completion,
ownership and completion refusals, and persisted records on an isolated host.
