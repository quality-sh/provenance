# Rust SDK fixture

This small app keeps production code unaware of Provenance. The spec
declares traceability with `provenance-sdk`; the test follows the built
spec's handles to the `expiry` rule and verifies ordinary production
code. `provenance_spec!` checks the identifier-to-key link at compile
time and `implemented_by!` checks the implementation path exists.

From this directory, with the Rust CLI built:

```sh
../../target/debug/provenance init --path . --scope default --path-prefix .
PROVENANCE_REPO="$PWD" cargo run --bin apply
PROVENANCE_REPO="$PWD" cargo test
../../target/debug/provenance rules list --format json
../../target/debug/provenance sdk verification-runs --format json
```

`cargo run --bin apply` reconciles the spec; `cargo test` runs the
typed verification in process. The engine links in as a library, so no
engine binary or environment handshake is involved.
