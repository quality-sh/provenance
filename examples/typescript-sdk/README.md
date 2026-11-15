# TypeScript SDK fixture

This small app keeps production code unaware of Provenance. The spec declares
traceability with `@quality-sh/provenance`; the test follows the built spec's
typed `requirements.sharing.rules.expiry` path and verifies ordinary production
code. Sources linked by `Requirement.from(...)` are collected by `build()`.

From this directory, with the Rust CLI and the `test-fixture` host built:

```sh
npm install
../../target/debug/provenance init --path . --scope default --path-prefix .
npm test
../../target/debug/provenance api rules --format json
../../target/debug/provenance api verification-runs --format json
```

`npm test` compiles the app. Its harness starts the source-checkout HTTP fixture
with this initialized repository, an opaque target ID, and a generated credential.
The app applies the spec and runs typed verification through that connection.
The harness stops the fixture after both consumers finish. Importing `provenance.spec.ts` alone has no
engine or persistence side effect.

The `file:../../packages/provenance` development dependency exercises normal
npm package resolution from the source checkout. Only the spec, the apply
script, and the test import it. `npm run test:production` builds the app,
installs a copy of it with `npm ci --omit=dev`, and loads `dist/share-links.js`
from that copy, where `@quality-sh/provenance` does not resolve. The
packed-install test covers the published package shape and its bundled engine
separately.

Build the fixture from the repository root with
`cargo build -p provenance-transport --features test-fixture --bin existing-root-host-fixture`.
This checks the connection-only SDK against real Rust storage. It does not test
an installed production host command; that command and its access policy remain
subject to the pending release decision. The SDK does not start a host.
